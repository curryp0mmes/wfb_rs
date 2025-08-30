use std::fs;
use std::os::unix::io::{AsRawFd, FromRawFd, OwnedFd};
use std::time::{Duration, Instant};
use std::mem::{size_of, zeroed};
use std::ffi::CString;

use crate::common::{self, get_ieee80211_header, Bandwidth};

pub struct Transmitter {
    _buffer_r: usize,
    _buffer_s: usize,
    log_interval: Duration,
    _k: u32,
    _n: u32,
    udp_port: u16,
    _fec_delay: u32,
    _debug_port: u16,
    _fec_timeout: u64,
    wifi_device: String,

    //private fields
    radiotap_header: Vec<u8>,
    ieee_sequence: u16,
    channel_id: u32,
}

impl Transmitter {
    pub fn new(
        radio_port: u8,
        link_id: u32,
        buffer_size_recv: usize,
        buffer_size_send: usize,
        log_interval: Duration,
        k: u32,
        n: u32,
        udp_port: u16,
        fec_delay: u32,
        bandwidth: Bandwidth,
        short_gi: bool,
        stbc: u8,
        ldpc: bool,
        mcs_index: u8,
        vht_mode: bool,
        vht_nss: u8,
        debug_port: u16,
        fec_timeout: u64,
        wifi_device: String,
    ) -> Self {
        let radiotap_header = common::get_radiotap_headers(
            stbc, ldpc, short_gi, bandwidth, mcs_index, vht_mode, vht_nss,
        );
        let link_id = link_id & 0xffffff;

        Self {
            _buffer_r: buffer_size_recv,
            _buffer_s: buffer_size_send,
            log_interval,
            _k: k,
            _n: n,
            udp_port,
            _fec_delay: fec_delay,
            _debug_port: debug_port,
            _fec_timeout: fec_timeout,
            wifi_device,
            radiotap_header,
            ieee_sequence: 0,
            channel_id: (link_id << 8) + (radio_port as u32),
        }
    }

    pub fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("Binding {} to Port {}", self.wifi_device, self.udp_port);
        
        let udp_file_descriptor = self.open_udp_socket().expect("Could not open udp port");
        let wifi_file_descriptor = self.open_raw_socket().expect("Error opening wifi socket");
        
        let mut log_time = Instant::now() + self.log_interval;
        let mut sent_packets = 0u32;
        let mut sent_bytes = 0u64;
        let mut received_packets = 0u32;

        loop {
            let timeout = log_time.saturating_duration_since(Instant::now());
            
            // Poll UDP socket
            let mut poll_fd = libc::pollfd {
                fd: udp_file_descriptor.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            
            let poll_result = unsafe {
                libc::poll(&mut poll_fd, 1, timeout.as_millis() as i32)
            };
            
            if poll_result < 0 {
                return Err("Poll error".into());
            }
            
            // Handle timeout
            if timeout.is_zero() {
                println!("Received {} packets, sent {} packets,\t\t {} bytes", received_packets, sent_packets, sent_bytes);
                sent_packets = 0;
                sent_bytes = 0;
                received_packets = 0;
                log_time = Instant::now() + self.log_interval;
            }
            
            if poll_result == 0 {
                continue; // Timeout, no data
            }
            
            // Read UDP data
            let mut buf = [0u8; 1500];
            let received = unsafe {
                libc::recv(udp_file_descriptor.as_raw_fd(), buf.as_mut_ptr() as *mut libc::c_void, buf.len(), libc::MSG_DONTWAIT)
            };
            
            if received == 0 {
                //TODO reset fec
                continue;
            }

            if received > 0 {
                received_packets += 1;
                let sent_size = self.send_packet(&wifi_file_descriptor, &buf[..received as usize])?;
                sent_bytes += sent_size as u64;
                sent_packets += 1;
            } else if received < 0 {
                let errno = unsafe { *libc::__errno_location() };
                if errno != libc::EAGAIN && errno != libc::EWOULDBLOCK {
                    eprintln!("recv error: errno {}", errno);
                }
            }
        }
    }
    
    fn open_udp_socket(&self) -> Result<OwnedFd, Box<dyn std::error::Error>> {
        let sockfd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) };
        if sockfd < 0 {
            return Err("Failed to create UDP socket".into());
        }
        
        let fd = unsafe { OwnedFd::from_raw_fd(sockfd) };
        
        // Set socket options
        let reuse_addr = 1i32;
        unsafe {
            libc::setsockopt(
                sockfd,
                libc::SOL_SOCKET,
                libc::SO_REUSEADDR,
                &reuse_addr as *const _ as *const libc::c_void,
                size_of::<i32>() as u32,
            );
        }
        
        // Bind socket
        let mut addr: libc::sockaddr_in = unsafe { zeroed() };
        addr.sin_family = libc::AF_INET as u16;
        addr.sin_port = (self.udp_port).to_be();
        addr.sin_addr.s_addr = libc::INADDR_ANY;
        
        let bind_result = unsafe {
            libc::bind(
                sockfd,
                &addr as *const _ as *const libc::sockaddr,
                size_of::<libc::sockaddr_in>() as u32,
            )
        };
        
        if bind_result < 0 {
            return Err("Failed to bind UDP socket".into());
        }
        
        Ok(fd)
    }
    
    fn open_raw_socket(&self) -> Result<OwnedFd, Box<dyn std::error::Error>> {
        let sockfd = unsafe { 
            libc::socket(libc::PF_PACKET, libc::SOCK_RAW, 0) 
        };

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
            
            let interface_type: u32 = type_content.trim().parse()
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
    
    fn send_packet(&mut self, wifi_fd: &OwnedFd, data: &[u8]) -> Result<usize, Box<dyn std::error::Error>> {
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
        
        let sent = unsafe {
            libc::sendmsg(wifi_fd.as_raw_fd(), &msg, 0)
        };
        
        if sent < 0 {
            let errno = unsafe { *libc::__errno_location() };
            if errno != libc::ENOBUFS {  // Ignore ENOBUFS
                eprintln!("sendmsg failed: errno {}", errno);
                return Err(format!("Failed to send packet: errno {}", errno).into());
            }
            return Ok(0);  // Treat ENOBUFS as non-fatal
        }
        
        Ok(sent as usize)
    }
}