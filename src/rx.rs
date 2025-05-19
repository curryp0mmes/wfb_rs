use nix::sys::socket::{self, SockProtocol, SockType, SockaddrIn};
use pcap::{self, Active, Capture};
use std::os::fd::{AsRawFd, OwnedFd};
use std::time::{Duration, Instant};

pub struct Receiver {
    client_address: String,
    client_port: u16,
    buffer_size: usize,
    log_interval: Duration,
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
        log_interval: Duration,
        wifi_device: String,
    ) -> Self {
        Self {
            client_address,
            client_port,
            buffer_size,
            log_interval,
            wifi_device,
            channel_id: link_id << 8 + radio_port,
        }
    }

    pub fn run(&self) {
        let udp_file_descriptor = self
            .open_udp_socket_output(self.buffer_size, SockType::Datagram, SockProtocol::Udp)
            .unwrap();
        let mut wifi_capture = self.open_socket_for_interface().unwrap();

        let mut log_time = Instant::now() + self.log_interval;

        loop {
            let time_until_next_log = log_time.saturating_duration_since(Instant::now());
            let poll_timeout = time_until_next_log.as_millis() as u16;

            let received_packet = wifi_capture.next_packet();
            //TODO process and send packet
            if time_until_next_log.is_zero() {
                //println!("Sent {} packets,\t\t {} bytes", sent_packets, sent_bytes);
                //sent_packets = 0;
                //sent_bytes = 0;
                log_time = Instant::now() + self.log_interval;
            }

            if received_packet.unwrap().len() == 0 {
                //TODO reset fec
                continue;
            }
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
            if let Err(e) =
                socket::setsockopt(&file_descriptor, socket::sockopt::SndBuf, &snd_buf_size)
            {
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

    fn open_socket_for_interface(&self) -> Result<Capture<Active>, nix::Error> {
        let wifi_max_size = 4045;

        let wifi_card: pcap::Device = pcap::Device::list()
            .unwrap()
            .iter()
            .find(|dev| dev.name == self.wifi_device)
            .unwrap()
            .clone();
        let cap = pcap::Capture::from_device(wifi_card)
            .unwrap()
            .snaplen(wifi_max_size + 256)
            .promisc(true)
            .timeout(-1)
            .immediate_mode(true)
            .open()
            .unwrap();
        let cap = cap.setnonblock();
        if let Err(e) = cap {
            println!("Error setting non-blocking mode: {}", e);
            return Err(nix::errno::Errno::EINVAL);
        }
        let mut cap = cap.unwrap();

        if cap.get_datalink() != pcap::Linktype::IEEE802_11_RADIOTAP {
            println!("Unknown encapsulation on interface {}", self.wifi_device);
            return Err(nix::errno::Errno::EINVAL);
        }

        if let Err(e) = cap.filter(
            format!(
                "ether[0x0a:2]==0x5742 && ether[0x0c:4] == {:#10x}",
                self.channel_id
            )
            .as_str(),
            true,
        ) {
            println!("Error setting filter: {}", e);
            return Err(nix::errno::Errno::EINVAL);
        }

        Ok(cap)
    }
}
