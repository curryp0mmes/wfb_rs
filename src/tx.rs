use nix::net::if_::if_nametoindex;
use nix::poll::{PollFlags, PollTimeout};
use nix::sys::socket::{
    self, AddressFamily, MsgFlags, SockFlag, SockProtocol, SockType, SockaddrIn, SockaddrLike,
    SockaddrStorage,
};
use nix::{cmsg_space, libc};
use std::io::{IoSlice, IoSliceMut};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd};
use std::time::{Duration, Instant};
use std::vec;

use crate::common::{self, get_ieee80211_header, Bandwidth};

pub struct Transmitter {
    buffer_size: usize,
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
        radio_port: u16,
        link_id: u32,
        buffer_size: usize,
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

        Self {
            buffer_size,
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
            channel_id: link_id << 8 + radio_port,
        }
    }

    pub fn run(&mut self) {
        println!("Binding {} to Port {}", self.wifi_device, self.udp_port);
        let udp_file_descriptor = open_udp_socket_for_rx(
            SockaddrIn::new(0, 0, 0, 0, self.udp_port),
            self.buffer_size,
            SockType::Datagram,
            SockProtocol::Udp,
        )
        .unwrap_or_else(|e| {
            println!("Error opening UDP socket: {:?}", e);
            std::process::exit(1);
        });

        let wificard_file_descriptor = open_socket_for_interface(self.wifi_device.as_str())
            .unwrap_or_else(|e| {
                println!("Error opening wifi socket: {:?}", e);
                println!("Make sure you have the right permissions, try running as root");
                std::process::exit(1);
            });

        println!(
            "UDP socket opened with fd: {}",
            udp_file_descriptor.as_raw_fd()
        );

        let mut log_time = Instant::now() + self.log_interval;

        let mut sent_packets: u32 = 0;
        let mut sent_bytes: u64 = 0;
        //TODO own thread for the udp socket polling (is it really needed?)
        loop {
            let time_until_next_log = log_time.saturating_duration_since(Instant::now());
            let poll_timeout = time_until_next_log.as_millis() as u16;

            let mut pollfds = vec![nix::poll::PollFd::new(
                udp_file_descriptor.as_fd(),
                PollFlags::POLLIN,
            )];
            let received_count: i32 =
                nix::poll::poll(&mut pollfds, PollTimeout::from(poll_timeout)).unwrap_or_else(
                    |e| {
                        println!("Error polling: {:?}", e);
                        std::process::exit(1);
                    },
                );

            if time_until_next_log.is_zero() {
                println!("Sent {} packets,\t\t {} bytes", sent_packets, sent_bytes);
                sent_packets = 0;
                sent_bytes = 0;
                log_time = Instant::now() + self.log_interval;
            }

            if received_count == 0 {
                //TODO reset fec
                continue;
            }

            let mut buf = [0u8; 1500]; // payload buffer
            let mut io_vector = [IoSliceMut::new(&mut buf)];

            let mut cmsg_space = cmsg_space!(u32);

            let msg = socket::recvmsg::<SockaddrStorage>(
                udp_file_descriptor.as_raw_fd(),
                &mut io_vector,
                Some(&mut cmsg_space),
                MsgFlags::MSG_DONTWAIT,
            )
            .unwrap_or_else(|e| {
                println!("Error receiving message: {:?}", e);
                std::process::exit(1);
            });

            let sent_size = self.send_packet(wificard_file_descriptor.as_fd(), msg);

            if let Err(e) = sent_size {
                println!("Error sending packet: {:?}", e);
            } else {
                let sent_size = sent_size.unwrap();
                if sent_size == 0 {
                    println!("No data sent");
                } else {
                    sent_bytes += sent_size as u64;
                    sent_packets += 1;
                }
            }
        }
    }

    fn send_packet(
        &mut self,
        file_descriptor: BorrowedFd,
        msg: socket::RecvMsg<SockaddrStorage>,
    ) -> Result<usize, nix::Error> {
        let ieee_header = get_ieee80211_header(0x08, self.channel_id, self.ieee_sequence);
        self.ieee_sequence += 16;

        let mut io_vector = vec![
            IoSlice::new(&self.radiotap_header),
            IoSlice::new(&ieee_header),
        ];
        for iov in msg.iovs() {
            io_vector.push(IoSlice::new(iov));
        }

        socket::sendmsg::<SockaddrStorage>(
            file_descriptor.as_raw_fd(),
            &io_vector,
            &[],
            MsgFlags::empty(),
            None,
        ) as Result<usize, nix::Error>
    }
}

fn open_udp_socket_for_rx(
    socket_address: SockaddrIn,
    rcv_buf_size: usize,
    socket_type: SockType,
    socket_protocol: SockProtocol,
) -> Result<OwnedFd, nix::Error> {
    // Create socket
    let file_descriptor = socket::socket(
        socket::AddressFamily::Inet,
        socket_type,
        socket::SockFlag::empty(),
        socket_protocol,
    )?;

    // Set SO_REUSEADDR
    socket::setsockopt(&file_descriptor, socket::sockopt::ReuseAddr, &true)?;

    // Set SO_RXQ_OVFL
    socket::setsockopt(&file_descriptor, socket::sockopt::RxqOvfl, &1)?;

    // Set SO_RCVBUF if specified
    if rcv_buf_size > 0 {
        socket::setsockopt(&file_descriptor, socket::sockopt::RcvBuf, &rcv_buf_size)?;
    }

    // Bind
    if let Err(e) = socket::bind(file_descriptor.as_raw_fd(), &socket_address) {
        let _ = drop(file_descriptor);
        return Err(e);
    }

    Ok(file_descriptor)
}

//TODO complete this function
fn open_socket_for_interface(interface_name: &str) -> Result<OwnedFd, nix::Error> {
    let file_descriptor = socket::socket(
        AddressFamily::Packet,
        SockType::Raw,
        SockFlag::empty(),
        SockProtocol::Raw,
    )?;

    //TODO Disable qdisc

    let ifindex = if_nametoindex(interface_name)
        .expect(format!("Interface {} not found", interface_name).as_str());

    assert!(ifindex > 0, "Invalid interface index");

    //TODO make this safe
    let sockaddr: SockaddrStorage = unsafe {
        let sa = libc::sockaddr_ll {
            sll_family: AddressFamily::Packet as u16,
            sll_protocol: SockProtocol::Raw as u16,
            sll_ifindex: ifindex as i32,
            sll_hatype: 0,
            sll_pkttype: 0,
            sll_halen: 0,
            sll_addr: [0; 8],
        };
        SockaddrStorage::from_raw(
            &sa as *const libc::sockaddr_ll as *const libc::sockaddr,
            Some(size_of::<libc::sockaddr_ll>() as u32),
        )
        .expect("Failed to create sockaddr_ll")
    };

    // Bind
    if let Err(e) = socket::bind(file_descriptor.as_raw_fd(), &sockaddr) {
        let _ = drop(file_descriptor);
        return Err(e);
    }

    Ok(file_descriptor)
}
