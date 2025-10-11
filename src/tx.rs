use raptorq::Encoder;
use std::ffi::CString;
use std::iter::once;
use std::mem::{size_of, zeroed};
use std::net::UdpSocket;
use std::os::unix::io::{AsRawFd, FromRawFd, OwnedFd};
use std::sync::mpsc::channel;
use std::time::Duration;
use std::{fs, io, thread};

use super::fec::{get_raptorq_oti, FecHeader};
use super::common::{self, get_ieee80211_header, Bandwidth};

pub struct Transmitter {
    tx: TX,
    fec: Option<TXFec>,
}

struct TX {
    wifi_socket: OwnedFd,
    radiotap_header: Vec<u8>,
    ieee_sequence: u16,
    channel_id: u32,
}

struct TXFec {
    block_id: u8,
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
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let radiotap_header = common::get_radiotap_headers(
            stbc, ldpc, short_gi, bandwidth, mcs_index, vht_mode, vht_nss,
        );
        let link_id = link_id & 0xffffff;

        let wifi_socket = TX::open_raw_socket(wifi_device)?;

        let channel_id = link_id << 8 | radio_port as u32;

        let tx = TX {
            wifi_socket,
            radiotap_header,
            ieee_sequence: 0,
            channel_id,
        };

        let fec = if fec_disabled {
            None
        } else { Some(TXFec {
            block_id: 0,
            pkg_indices: Vec::new(),
            block_buffer: Vec::new(),
            min_block_size,
            wifi_packet_size,
            redundant_pkgs
        })};

        Ok(Self {
            tx,
            fec
        })
    }

    pub fn run(mut self, source_port: u16, buffer_r: usize, log_interval: Duration) -> Result<(), Box<dyn std::error::Error>> {

        let udp_socket = UdpSocket::bind(format!("0.0.0.0:{}", source_port))?;
        
        let (sent_bytes_s, sent_bytes_r) = channel();
        let (received_bytes_s, received_bytes_r) = channel();

        // start logtask
        thread::spawn(move || {
            loop {
                let (sent_packets, sent_bytes): (u32, u32) = sent_bytes_r.try_iter().fold((0, 0), |(count, sum), v| (count + 1, sum + v));
                let (received_packets, received_bytes): (u32, u32) = received_bytes_r.try_iter().fold((0, 0), |(count, sum), v| (count + 1, sum + v));
                println!(
                    "Packets R->T {}->{},\tBytes {}->{}",
                    sent_packets,
                    received_packets,
                    received_bytes,
                    sent_bytes,
                );
                thread::sleep(log_interval);
            }
        });

        let (block_s, block_r) = channel::<Vec<Vec<u8>>>();
        
        thread::spawn(move || {
            for block in block_r.into_iter() {
                for packet in block.into_iter() {
                    let sent = self.tx.send_packet(&packet).unwrap() as u32;
                    if sent < packet.len() as u32 {
                        eprintln!("socket dropped some bytes");
                    }
                    sent_bytes_s.send(sent as u32).unwrap();
                }
            }
        });

        loop {
            let mut udp_recv_buffer = vec![0u8; buffer_r];
            let poll_result = udp_socket.recv(&mut udp_recv_buffer);

            match poll_result {
                Err(err) => match err.kind() {
                    io::ErrorKind::TimedOut => continue,
                    err => {
                        eprintln!("Error polling udp input: {}", err);
                        continue;
                    },
                },
                Ok(received) => {
                    if received == 0 {
                        //Empty packet
                        eprintln!("Empty packet");
                        continue;
                    }
                    if received == buffer_r {
                        eprintln!("Input packet seems too large");
                    }
                    
                    let udp_packet = &udp_recv_buffer[..received];

                    received_bytes_s.send(received as u32)?;

                    if let Some(fec) = self.fec.as_mut() {
                        if let Some(block) = fec.process_packet_fec(udp_packet) {
                            block_s.send(block)?;
                        }
                    } else {
                        // if fec is disabled just immediately return the raw block
                        block_s.send(vec![udp_packet.to_vec()])?;
                    }
                }
            }
            
        }
    }
}
impl TXFec {
    fn process_packet_fec(&mut self, packet: &[u8]) -> Option<Vec<Vec<u8>>> {
        // wait for block buffer to fill
        self.pkg_indices.push(self.block_buffer.len() as u16);
        self.block_buffer.extend_from_slice(packet);
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

            let header = FecHeader::new(self.block_id, block_size, self.wifi_packet_size).to_bytes();
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
impl TX {
    fn open_raw_socket(wifi_device: String) -> Result<OwnedFd, Box<dyn std::error::Error>> {
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
        let ifname = CString::new(wifi_device.as_str())?;
        let ifindex = unsafe { libc::if_nametoindex(ifname.as_ptr()) };

        if ifindex == 0 {
            return Err(format!("Interface {} not found", wifi_device).into());
        }

        //Check if wifi card is in monitor mode
        {
            let type_path = format!("/sys/class/net/{}/type", wifi_device);
            let type_content = fs::read_to_string(&type_path)
                .map_err(|_| format!("Interface {} not found or inaccessible", wifi_device))?;

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

        let sent = unsafe { libc::sendmsg(self.wifi_socket.as_raw_fd(), &msg, 0) };

        if sent < 0 {
            let errno = unsafe { *libc::__errno_location() };
            if errno != libc::ENOBUFS {
                // Ignore ENOBUFS
                eprintln!("sendmsg failed: errno {}", errno);
                return Err(format!("Failed to send packet: errno {}", errno).into());
            }
            return Ok(0); // Treat ENOBUFS as non-fatal
        }

        let header_len = self.radiotap_header.len() + ieee_header.len();

        Ok((sent as usize).saturating_sub(header_len))
    }
}
