use nix::{cmsg_space, libc};
use nix::net::if_::if_nametoindex;
use nix::poll::{PollFlags, PollTimeout};
use nix::sys::socket::{
    self, AddressFamily, MsgFlags, SockFlag, SockProtocol, SockType, SockaddrIn, SockaddrLike, SockaddrStorage
};
use std::fmt::format;
use std::io::{IoSlice, IoSliceMut};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd};
use std::time::{Duration, Instant};
use std::vec;

pub struct Receiver {
    client_address: String,
    client_port: u16,
    radio_port: u16,
    buffer_size: usize,
    log_interval: u64,
    wifi_device: String,
    channel_id: u32,
}

impl Receiver {
    pub fn new(
        client_address: String,
        client_port: u16,
        radio_port: u16,
        link_id: u32,
        buffer_size: usize,
        log_interval: u64,
        wifi_device: String,
    ) -> Self {
        Self {
            client_address,
            client_port,
            radio_port,
            buffer_size,
            log_interval,
            wifi_device,
            channel_id: link_id << 8 + radio_port,
        }
    }

    pub fn run(&self) {
        let udp_file_descriptor = self.open_udp_socket_output(
            self.buffer_size,
            SockType::Datagram,
            SockProtocol::Udp,
        );

        loop {

        }
    }


    fn open_udp_socket_output(
        &self,
        snd_buf_size: usize,
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

        if snd_buf_size > 0 {
            if let Err(e) = socket::setsockopt(&file_descriptor, socket::sockopt::SndBuf, &snd_buf_size) {
                drop(file_descriptor);
                return Err(e);
            }
        }
    
        let compound_ouput_address = format!("{}:{}", self.client_address, self.client_port);
        let socket_address: SockaddrIn = compound_ouput_address.parse().unwrap();

        // Bind
        if let Err(e) = socket::bind(file_descriptor.as_raw_fd(), &socket_address) {
            let _ = drop(file_descriptor);
            return Err(e);
        }
    
        Ok(file_descriptor)
    }
}
