use raptorq::Encoder;
use std::collections::HashMap;
use std::ffi::CString;
use std::mem::{size_of, zeroed};
use std::net::UdpSocket;
use std::os::unix::io::{AsRawFd, FromRawFd, OwnedFd};
use std::time::{Duration, Instant};
use std::{fs, io};

use crate::common::get_fec_oti;
use crate::common::{self, get_ieee80211_header, Bandwidth};
use crate::common::{FecHeader, FEC_HEADER_SIZE};

pub struct Transmitter {
    buffer_r: usize,
    _buffer_s: usize,
    log_interval: Duration,
    udp_port: u16,
    _debug_port: u16,
    wifi_device: String,

    //private fields
    radiotap_header: Vec<u8>,
    ieee_sequence: u16,
    channel_id: u32,
    buf: Vec<u8>,
    input_udp_socket: UdpSocket,
    block_id: u32,
    send_buffer: HashMap<u32, Vec<Vec<u8>>>,

    _encoder_cache: Option<Encoder>,
    _fec_packet_buffer: Vec<Vec<u8>>,
    _k: u32,
    _n: u32,
    _fec_delay: u32,
    _fec_timeout: u64,
    fec_enabled: bool,

    time_benchmark: Instant,
}

impl Transmitter {
    pub fn new(
        radio_port: u8,
        link_id: u32,
        buffer_size_recv: usize,
        buffer_size_send: usize,
        log_interval: Duration,
        udp_port: u16,
        bandwidth: Bandwidth,
        short_gi: bool,
        stbc: u8,
        ldpc: bool,
        mcs_index: u8,
        vht_mode: bool,
        vht_nss: u8,
        debug_port: u16,
        wifi_device: String,
        k: u32,
        n: u32,
        fec_delay: u32,
        fec_timeout: u64,
        fec_enabled: bool,
    ) -> Self {
        let radiotap_header = common::get_radiotap_headers(
            stbc, ldpc, short_gi, bandwidth, mcs_index, vht_mode, vht_nss,
        );
        let link_id = link_id & 0xffffff;

        Self {
            buffer_r: buffer_size_recv,
            _buffer_s: buffer_size_send,
            log_interval,
            udp_port,
            _debug_port: debug_port,
            wifi_device,

            radiotap_header,
            ieee_sequence: 0,
            channel_id: (link_id << 8) + (radio_port as u32),
            buf: vec![0u8; buffer_size_recv],
            input_udp_socket: UdpSocket::bind(format!("0.0.0.0:{}", udp_port))
                .expect("Could not open udp port"),
            send_buffer: HashMap::new(),
            block_id: 0,

            _k: k,
            _n: n,
            _fec_delay: fec_delay,
            _fec_timeout: fec_timeout,
            fec_enabled,
            _encoder_cache: None,
            _fec_packet_buffer: Vec::new(),

            time_benchmark: Instant::now(),
        }
    }

    pub fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("Binding {} to Port {}", self.wifi_device, self.udp_port);

        self.input_udp_socket.set_nonblocking(true)?;

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

            self.poll_incoming(&mut received_packets, &mut received_bytes);

            // Collect packets to send to avoid double mutable borrow
            let mut packets_to_send = Vec::new();
            let mut remove_ids = Vec::new();
            for (id, block) in &mut self.send_buffer {
                match block.pop() {
                    Some(data) => {
                        packets_to_send.push((id.clone(), data));
                        block.truncate(2);
                    }
                    None => {
                        remove_ids.push(*id);
                    }
                }
            }
            for (_id, data) in packets_to_send {
                let sent_size = self.send_packet(&wifi_file_descriptor, &data)?;
                sent_bytes += sent_size as u64;
                sent_packets += 1;
                self.poll_incoming(&mut received_packets, &mut received_bytes);
            }
            for id in remove_ids {
                self.send_buffer.remove(&id);
            }

            //println!("Wifi Done \t{} ns",self.time_benchmark.elapsed().as_nanos());
        }
    }

    fn poll_incoming(&mut self, received_packets: &mut u32, received_bytes: &mut u64) {
        self.time_benchmark = Instant::now();
        let poll_result = self.input_udp_socket.recv_from(&mut self.buf);

        match poll_result {
            Err(err) => match err.kind() {
                io::ErrorKind::WouldBlock => return,
                io::ErrorKind::TimedOut => return,
                _ => panic!("Error polling udp input: {}", err),
            },
            Ok((received, _origin)) => {
                if received == 0 {
                    //Empty packet, //TODO reset fec
                    return;
                }

                if received == self.buffer_r {
                    println!("Input packet seems too large");
                }
                *received_packets += 1;
                *received_bytes += received as u64;

                //println!("\nPolling {} b \t{} ns", received, self.time_benchmark.elapsed().as_nanos());

                let block = if self.fec_enabled {
                    // Optimized FEC: Generate fewer repair packets and batch operations
                    let encoder = Encoder::new(&self.buf[..received as usize], get_fec_oti());

                    // Generate minimal repair packets for maximum performance
                    let repair_packets = 1; // Minimal repair for best performance
                    let encoded_packets = encoder.get_encoded_packets(repair_packets);

                    //println!("Encoding \t{} ns", self.time_benchmark.elapsed().as_nanos());

                    // Pre-allocate combined buffer to avoid repeated allocations

                    let mut block: Vec<Vec<u8>> = Vec::new();
                    for (packet_id, packet) in encoded_packets.iter().enumerate() {
                        let fec_header =
                            FecHeader::new(self.block_id, packet_id as u32, received as u32);
                        let fec_header_bytes = fec_header.to_bytes();
                        let serialized_packet = packet.serialize();

                        // Create combined payload
                        let mut combined_payload =
                            Vec::with_capacity(FEC_HEADER_SIZE + serialized_packet.len());
                        combined_payload.extend_from_slice(&fec_header_bytes);
                        combined_payload.extend_from_slice(&serialized_packet);

                        block.push(combined_payload);
                    }
                    block
                } else {
                    //else wrap the raw udp stream into another vector of length one (only gets sent once)
                    vec![Vec::from(&self.buf[..received as usize])]
                };

                self.block_id = self.block_id.wrapping_add(1);
                self.send_buffer.insert(self.block_id, block);

                //println!("Writing blocks \t{} ns", self.time_benchmark.elapsed().as_nanos());
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
