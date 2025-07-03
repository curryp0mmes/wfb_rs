use pcap::{self, Active, Capture, Packet};
use radiotap::Radiotap;
use std::net::UdpSocket;
use std::time::{Duration, Instant};

use crate::common;

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
        let compound_ouput_address = format!("{}:{}", self.client_address, self.client_port);

        let udp_socket = UdpSocket::bind("0.0.0.0:5601").unwrap();
        let connection_output = udp_socket.connect(compound_ouput_address);
        if let Err(e) = connection_output {
            println!("Error setting output udp address: {:?}", e);
            return;
        }
        let mut wifi_capture: Capture<Active> = self.open_socket_for_interface().unwrap();

        let mut log_time = Instant::now() + self.log_interval;
        let mut received_packets_count = 0u64;
        loop {
            let time_until_next_log = log_time.saturating_duration_since(Instant::now());
            //let poll_timeout = time_until_next_log.as_millis() as u16;

            let received_packet: Result<Packet<'_>, pcap::Error> = wifi_capture.next_packet();

            if let Err(e) = received_packet {
                if e != pcap::Error::TimeoutExpired {
                    println!("Error receiving packet: {:?}", e);
                    continue;
                }
            } else {
                let packet = received_packet.unwrap();
                if packet.len() != 0 {
                    let radiotap_header = Radiotap::from_bytes(packet.data).unwrap(); // parses the first n bytes as header

                    // TODO process packet
                    received_packets_count += 1;
                    println!("Received packet: {:?}", radiotap_header);

                    let header_len = radiotap_header.header.length as usize;
                    let rest_of_packet: &[u8] = &packet.data[header_len..];

                    if rest_of_packet.len() > common::IEEE80211_HEADER.len() {
                        let rest_of_packet: &[u8] =
                            &rest_of_packet[common::IEEE80211_HEADER.len()..];
                        let result = udp_socket.send(rest_of_packet);
                        if let Err(e) = result {
                            println!("Error forwarding packet, {:?}", e);
                        }
                    }
                } else {
                    //len == 0
                    //TODO reset fec
                    continue;
                }
            }
            if time_until_next_log.is_zero() {
                println!("Received {} packets", received_packets_count);
                received_packets_count = 0;
                //println!("Sent {} packets,\t\t {} bytes", sent_packets, sent_bytes);
                //sent_packets = 0;
                //sent_bytes = 0;
                log_time = Instant::now() + self.log_interval;
            }
        }
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
