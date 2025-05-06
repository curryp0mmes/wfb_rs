use std::os::fd::AsRawFd;
use std::vec;
use nix::net::if_::if_nametoindex;
use nix::sys::ioctl;
use nix::sys::socket::{self, SockaddrIn, UnixAddr};
use nix::unistd::close;
use std::net::Ipv4Addr;
use std::os::unix::io::RawFd;


pub struct Transmitter {
    radio_port: u16,
    buffer_size: usize,
    log_interval: u64,
    k: u32,
    n: u32,
    udp_port: u16,
    fec_delay: u32,
    bandwidth: u32,
    short_gi: bool,
    stbc: u32,
    ldpc: u32,
    mcs_index: u32,
    vht_nss: u32,
    debug_port: u16,
    fec_timeout: u64,
    wifi_device: String,
}

impl Transmitter {
    pub fn new(
        radio_port: u16,
        buffer_size: usize,
        log_interval: u64,
        k: u32,
        n: u32,
        udp_port: u16,
        fec_delay: u32,
        bandwidth: u32,
        short_gi: bool,
        stbc: u32,
        ldpc: u32,
        mcs_index: u32,
        vht_nss: u32,
        debug_port: u16,
        fec_timeout: u64,
        wifi_device: String,
    ) -> Self {
        Self {
            radio_port,
            buffer_size,
            log_interval,
            k,
            n,
            udp_port,
            fec_delay,
            bandwidth,
            short_gi,
            stbc,
            ldpc,
            mcs_index,
            vht_nss,
            debug_port,
            fec_timeout,
            wifi_device,
        }
    }

    pub fn run(&self) {
        println!("Binding {} to Port {}", self.wifi_device, self.udp_port);
        let udp_file_descriptor = match open_udp_socket_for_rx(
            socket::SockaddrIn::new(0,0,0,0, self.udp_port),
            self.buffer_size,
            0, // Bind to all interfaces
            socket::SockType::Datagram,
            socket::SockProtocol::Udp,
        ) {
            Ok(fd) => fd,
            Err(e) => {
                println!("Error opening UDP socket: {:?}", e);
                return;
            }
        };

        println!("UDP socket opened with fd: {}", udp_file_descriptor);

        assert!(self.buffer_size > 0);
        let mut buffer: Vec<u8> = vec![0; self.buffer_size];
        //TODO own thread for the udp socket polling
        loop {
            
        }
    }
}


fn open_udp_socket_for_rx(
    socket_address: SockaddrIn,
    rcv_buf_size: usize,
    bind_addr: u32,
    socket_type: socket::SockType,
    socket_protocol: socket::SockProtocol,
) -> Result<RawFd, nix::Error> {
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
        let _ = close(file_descriptor);
        return Err(e);
    }

    Ok(file_descriptor.as_raw_fd())
}

/*TODO 
fn open_socket_for_interface(

) -> Result<RawFd, nix::Error> {

    let interface_name = "wlan0";

    let file_descriptor = socket::socket(
        socket::AddressFamily::Packet,
        socket::SockType::Raw,
        socket::SockFlag::empty(),
        socket::SockProtocol::Raw,
    )?;

    let ifindex = if_nametoindex(interface_name).expect("Failure");

    let sockaddr = socket::
    socket::bind(
        file_descriptor.as_raw_fd(),
        sockaddr,
        
    )?;
    



    Ok(file_descriptor.as_raw_fd())
}

*/