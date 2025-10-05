use raptorq::Encoder;
use std::ffi::CString;
use std::iter::once;
use std::mem::{size_of, zeroed};
use std::net::UdpSocket;
use std::os::unix::io::{AsRawFd, FromRawFd, OwnedFd};
use std::time::{Duration, Instant};
use std::{fs, io};

use super::fec::{get_raptorq_oti, FecHeader};
use super::common::{self, get_ieee80211_header, Bandwidth};

pub struct Transmitter {
    buffer_r: usize,
    log_interval: Duration,
    udp_port: u16,
    wifi_device: String,

    radiotap_header: Vec<u8>,
    ieee_sequence: u16,
    channel_id: u32,
    block_id: u8,
    input_udp_socket: UdpSocket,

    fec_disabled: bool,
    pkg_indices: Vec<u16>,
    block_buffer: Vec<u8>,
    min_block_size: u16,
    wifi_packet_size: u16,
    redundant_pkgs: u32,
}

impl Transmitter {
    pub fn new(
        radio_port: u8,
        link_id: u32,
        buffer_size_recv: usize,
        log_interval: Duration,
        udp_port: u16,
        bandwidth: Bandwidth,
        short_gi: bool,
        stbc: u8,
        ldpc: bool,
        mcs_index: u8,
        vht_mode: bool,
        vht_nss: u8,
        wifi_device: String,
        fec_disabled: bool,
        min_block_size: u16,
        wifi_packet_size: u16,
        redundant_pkgs: u32,
    ) -> Self {
        let radiotap_header = common::get_radiotap_headers(
            stbc, ldpc, short_gi, bandwidth, mcs_index, vht_mode, vht_nss,
        );
        let link_id = link_id & 0xffffff;

        Self {
            buffer_r: buffer_size_recv,
            log_interval,
            udp_port,
            wifi_device,

            radiotap_header,
            ieee_sequence: 0,
            channel_id: (link_id << 8) | (radio_port as u32),
            input_udp_socket: UdpSocket::bind(format!("0.0.0.0:{}", udp_port))
                .expect("Could not open udp port"),
            block_id: 0,

            fec_disabled,
            pkg_indices: Vec::new(),
            block_buffer: Vec::new(),
            min_block_size,
            wifi_packet_size,
            redundant_pkgs
        }
    }

    pub fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("Binding {} to Port {}", self.wifi_device, self.udp_port);

        let wifi_file_descriptor = self.open_raw_socket().expect("Error opening wifi socket");

        let mut log_time = Instant::now() + self.log_interval;
        let mut sent_packets = 0u32;
        let mut sent_bytes = 0u64;
        let mut received_packets = 0u32;
        let mut received_bytes = 0u64;

        loop {
            let timeout = log_time.saturating_duration_since(Instant::now());

            // Log data if timeout has passed
            if timeout.is_zero() && !self.log_interval.is_zero() {
                println!(
                    "Packets R->T {}->{},\t\tBytes: {}->{}",
                    received_packets, sent_packets, received_bytes, sent_bytes
                );
                sent_packets = 0;
                sent_bytes = 0;
                received_packets = 0;
                received_bytes = 0;
                log_time = log_time + self.log_interval;
                continue;
            }

            if let Some(block) = self.poll_incoming(&mut received_packets, &mut received_bytes) {
                for packet in block {
                    let send = self.send_packet(&wifi_file_descriptor, &packet)? as u64;
                    if send < packet.len() as u64 {
                        eprintln!("socket dropped some bytes");
                    }
                    sent_bytes += send;
                    sent_packets += 1;
                }
            }
        }
    }

    fn poll_incoming(&mut self, received_packets: &mut u32, received_bytes: &mut u64) -> Option<Vec<Vec<u8>>> {
        let mut udp_recv_buffer = vec![0u8; self.buffer_r];
        let poll_result = self.input_udp_socket.recv(&mut udp_recv_buffer);

        match poll_result {
            Err(err) => match err.kind() {
                io::ErrorKind::TimedOut => return None,
                err => {
                    eprintln!("Error polling udp input: {}", err);
                    return None;
                },
            },
            Ok(received) => {
                if received == 0 {
                    //Empty packet
                    eprintln!("Empty packet");
                    return None;
                }
                if received == self.buffer_r {
                    eprintln!("Input packet seems too large");
                }
                // for debug
                *received_packets += 1;
                *received_bytes += received as u64;
                
                // if fec is disabled just immediately return raw data
                if self.fec_disabled {
                    return Some(vec![udp_recv_buffer[..received].to_vec()])
                }
                
                // wait for block buffer to fill
                self.pkg_indices.push(self.block_buffer.len() as u16);
                self.block_buffer.extend_from_slice(&udp_recv_buffer[..received]);
                if self.block_buffer.len() < self.min_block_size as usize {
                    return None;
                }
                
                // add udp package limiter info header (append it for performance)
                let mut udp_pkgs_header: Vec<_> = self.pkg_indices
                    .iter()
                    .map(|i| i.to_le_bytes())
                    .flatten()
                    .chain(once(self.pkg_indices.len() as u8))
                    .collect();

                self.block_buffer.append(&mut udp_pkgs_header);

                let block_size = self.block_buffer.len() as u16;

                // if block is full, return it
                let block = {
                    let oci = get_raptorq_oti(block_size, self.wifi_packet_size);
                    let encoder = Encoder::new(&self.block_buffer, oci);

                    let header = FecHeader::new(self.block_id, block_size).to_bytes();
                    encoder.get_encoded_packets(self.redundant_pkgs)
                        .iter()
                        .map(|p| [&header, &p.serialize()[..]].concat())
                        .collect()

                };

                self.block_id = self.block_id.wrapping_add(1);
                self.block_buffer.clear();
                self.pkg_indices.clear();
                Some(block)
            }
        }
    }

    fn open_raw_socket(&self) -> Result<OwnedFd, Box<dyn std::error::Error>> {
        let sockfd = unsafe { libc::socket(libc::PF_PACKET, libc::SOCK_RAW, 0) };

        if sockfd < 0 {
            return Err("Failed to create raw socket, you need root privileges to do so. Try again with sudo!".into());
        }

        // Set PACKET_QDISC_BYPASS
        let bypass = 1i32;
        unsafe {
            libc::setsockopt(
                sockfd,
                libc::SOL_PACKET,
                libc::PACKET_QDISC_BYPASS,
                &bypass as *const _ as *const libc::c_void,
                size_of::<i32>() as u32,
            );
        }

        // Get interface index
        let ifname = CString::new(self.wifi_device.as_str())?;
        let ifindex = unsafe { libc::if_nametoindex(ifname.as_ptr()) };

        if ifindex == 0 {
            return Err(format!("Interface {} not found", self.wifi_device).into());
        }

        //Check if wifi card is in monitor mode
        {
            let type_path = format!("/sys/class/net/{}/type", self.wifi_device);
            let type_content = fs::read_to_string(&type_path)
                .map_err(|_| format!("Interface {} not found or inaccessible", self.wifi_device))?;

            let interface_type: u32 = type_content
                .trim()
                .parse()
                .map_err(|_| "Failed to parse interface type")?;

            // ARPHRD_IEEE80211_RADIOTAP = 803 (monitor mode)
            // ARPHRD_ETHER = 1 (managed mode)
            // ARPHRD_IEEE80211 = 801 (other 802.11 modes)
            if interface_type != 803 {
                return Err("Wifi Device is not in monitor mode".into());
            }
        }

        // Bind to interface
        let mut addr: libc::sockaddr_ll = unsafe { zeroed() };
        addr.sll_family = libc::AF_PACKET as u16;
        addr.sll_protocol = 0; // We'll specify protocol per packet
        addr.sll_ifindex = ifindex as i32;

        let bind_result = unsafe {
            libc::bind(
                sockfd,
                &addr as *const _ as *const libc::sockaddr,
                size_of::<libc::sockaddr_ll>() as u32,
            )
        };

        if bind_result < 0 {
            return Err("Failed to bind raw socket".into());
        }

        let fd = unsafe { OwnedFd::from_raw_fd(sockfd) };

        Ok(fd)
    }

    fn send_packet(
        &mut self,
        wifi_fd: &OwnedFd,
        data: &[u8],
    ) -> Result<usize, Box<dyn std::error::Error>> {
        // Create IEEE 802.11 and radiotap headers
        let ieee_header = get_ieee80211_header(0x08, self.channel_id, self.ieee_sequence);
        self.ieee_sequence += 16;

        // Assemble payload from headers and data
        let iovecs = [
            libc::iovec {
                iov_base: self.radiotap_header.as_ptr() as *mut libc::c_void,
                iov_len: self.radiotap_header.len(),
            },
            libc::iovec {
                iov_base: ieee_header.as_ptr() as *mut libc::c_void,
                iov_len: ieee_header.len(),
            },
            libc::iovec {
                iov_base: data.as_ptr() as *mut libc::c_void,
                iov_len: data.len(),
            },
        ];

        let msg: libc::msghdr = libc::msghdr {
            msg_name: std::ptr::null_mut(),
            msg_namelen: 0,
            msg_iov: iovecs.as_ptr() as *mut libc::iovec,
            msg_iovlen: iovecs.len(),
            msg_control: std::ptr::null_mut(),
            msg_controllen: 0,
            msg_flags: 0,
        };

        let sent = unsafe { libc::sendmsg(wifi_fd.as_raw_fd(), &msg, 0) };

        if sent < 0 {
            let errno = unsafe { *libc::__errno_location() };
            if errno != libc::ENOBUFS {
                // Ignore ENOBUFS
                eprintln!("sendmsg failed: errno {}", errno);
                return Err(format!("Failed to send packet: errno {}", errno).into());
            }
            return Ok(0); // Treat ENOBUFS as non-fatal
        }

        Ok(sent as usize)
    }
}
