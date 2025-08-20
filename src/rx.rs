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

    pub fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let compound_output_address = format!("{}:{}", self.client_address, self.client_port);
        
        let udp_socket = UdpSocket::bind("0.0.0.0:0")?; // Bind to any available port
        udp_socket.connect(&compound_output_address)?;
        
        let mut wifi_capture = self.open_wifi_capture()?;

        let mut log_time = Instant::now() + self.log_interval;
        let mut received_packets_count = 0u64;
        let mut processed_packets_count = 0u64;

        loop {

            match wifi_capture.next_packet() {
                Ok(packet) if packet.len() > 0 => {
                    received_packets_count += 1;
                    
                    if let Some(payload) = self.process_packet(&packet)? {
                        if let Err(e) = udp_socket.send(&payload) {
                            eprintln!("Error forwarding packet: {}", e);
                        } else {
                            processed_packets_count += 1;
                        }
                    }
                }
                Ok(packet) if packet.len() == 0 => {
                    //TODO reset fec
                    continue;
                }
                Ok(_) => {
                    // Empty packet, continue
                    continue;
                }
                Err(pcap::Error::TimeoutExpired) => {
                    // Timeout is normal, continue
                    continue;
                }
                Err(e) => {
                    eprintln!("Error receiving packet: {}", e);
                    continue;
                }
            }

            let now = Instant::now();
            if now >= log_time {
                println!("Received {} packets, processed {}", received_packets_count, processed_packets_count);
                received_packets_count = 0;
                processed_packets_count = 0;
                log_time = now + self.log_interval;
            }
        }
    }

    fn process_packet(&self, packet: &Packet) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error>> {
        let data = packet.data;

        if data.len() < 4 {
            return Ok(None); // Too short for radiotap header
        }

        // Parse minimal radiotap header to get length
        let radiotap_len = u16::from_le_bytes([data[2], data[3]]) as usize;

        //Parse the whole radiotap header via library
        let _radiotap_header = Radiotap::from_bytes(data)?;

        //println!("Received header: {:?}", radiotap_header);
        //let header_len = radiotap_header.header.size;

        // Skip radiotap header and IEEE 802.11 header
        let payload_start = radiotap_len + common::IEEE80211_HEADER.len();
        
        if data.len() <= payload_start {
            return Ok(None); // No payload
        }

        let payload = &data[payload_start..];
        
        if payload.len() > self.buffer_size {
            return Ok(None); // Payload too large
        }

        // Basic validation - check if this looks like a WFB packet
        if payload.len() < 10 {
            return Ok(None); // Too short for WFB packet
        }

        Ok(Some(payload.to_vec()))
    }

    fn open_wifi_capture(&self) -> Result<Capture<Active>, Box<dyn std::error::Error>> {
        let wifi_max_size = 4096;

        let wifi_card = pcap::Device::list()?
            .into_iter()
            .find(|dev| dev.name == self.wifi_device)
            .ok_or_else(|| format!("WiFi device {} not found", self.wifi_device))?;

        let mut cap = pcap::Capture::from_device(wifi_card)?
            .snaplen(wifi_max_size)
            .promisc(true)
            .timeout(100) // 100ms timeout instead of -1
            .immediate_mode(true)
            .open()?;

        if cap.get_datalink() != pcap::Linktype::IEEE802_11_RADIOTAP {
            return Err(format!("Unknown encapsulation on interface {}", self.wifi_device).into());
        }

        // Set the BPF filter to match the original C++ code
        let filter = format!("ether[0x0a:2]==0x5742 && ether[0x0c:4] == {:#010x}", self.channel_id);
        cap.filter(&filter, true)?;

        Ok(cap)
    }
}
