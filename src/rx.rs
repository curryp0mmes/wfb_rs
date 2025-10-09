use pcap::{self, Active, Capture, Packet};
use radiotap::Radiotap;
use raptorq::{Decoder, EncodingPacket};
use std::collections::{HashMap, HashSet};
use std::iter::once;
use std::net::UdpSocket;
use std::time::{Duration, Instant};

use super::fec::{get_raptorq_oti, FecHeader, FEC_HEADER_SIZE};
use super::common;

pub struct Receiver {
    client_address: String,
    client_port: u16,
    buffer_size: usize,
    log_interval: Duration,
    wifi_devices: Vec<String>,
    channel_id: u32,

    fec_decoders: HashMap<u8, Decoder>,
    decoded_blocks: HashSet<u8>,
}

impl Receiver {
    pub fn new(
        client_address: String,
        client_port: u16,
        radio_port: u16,
        link_id: u32,
        buffer_size: usize,
        log_interval: Duration,
        wifi_devices: Vec<String>,
    ) -> Self {
        Self {
            client_address,
            client_port,
            buffer_size,
            log_interval,
            wifi_devices,
            channel_id: link_id << 8 | radio_port as u32,

            fec_decoders: HashMap::new(),
            decoded_blocks: HashSet::new(),
        }
    }

    pub fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let compound_output_address = format!("{}:{}", self.client_address, self.client_port);

        let udp_socket = UdpSocket::bind("0.0.0.0:0")?; // Bind to any available port
        udp_socket.connect(&compound_output_address)?;

        let mut wifi_captures: Vec<Capture<Active>> = self
            .wifi_devices
            .iter()
            .map(|dev| self.open_wifi_capture(dev.clone()))
            .collect::<Result<_, _>>()?;

        let mut log_time = Instant::now() + self.log_interval;
        let mut received_packets = 0u64;
        let mut received_bytes = 0u64;
        let mut sent_packets = 0u64;
        let mut sent_bytes = 0u64;
        println!("Receiver ready!");

        loop {
            if Instant::now() >= log_time {
                println!(
                    "Packets R->T {}->{},\tBytes {}->{}",
                    received_packets, sent_packets, received_bytes, sent_bytes
                );
                received_packets = 0;
                received_bytes = 0;
                sent_packets = 0;
                sent_bytes = 0;
                log_time = log_time + self.log_interval;
            }

            for wifi_capture in &mut wifi_captures {
                match wifi_capture.next_packet() {
                    Ok(packet) if packet.len() > 0 => {
                        if let Some(payload) = self.process_packet(&packet)? {
                            received_packets += 1;
                            received_bytes += payload.len() as u64;
                            if let Some(fec_header) = FecHeader::from_bytes(&payload) {
                                if let Some(decoded_data) = self.process_fec_packet(fec_header, &payload[FEC_HEADER_SIZE..]) {
                                    for udp_pkg in decoded_data {
                                        match udp_socket.send(&udp_pkg) {
                                            Err(e) => {
                                                eprintln!("Error forwarding packet: {}", e);
                                            }
                                            Ok(sent) => {
                                                sent_packets += 1;
                                                sent_bytes += sent as u64;
                                            }
                                        }
                                    }
                                }
                            } else {
                                // Forward packet directly without FEC processing
                                match udp_socket.send(&payload) {
                                    Err(e) => {
                                        eprintln!("Error forwarding packet: {}", e);
                                    }
                                    Ok(sent) => {
                                        sent_packets += 1;
                                        sent_bytes += sent as u64;
                                    }
                                }
                            }
                        } else {
                            eprintln!("could not decode packet");
                        }
                    }
                    Ok(_packet) => {
                        //TODO reset fec (?)
                        eprintln!("packet len <= 0");
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
            }
        }
    }

    fn process_fec_packet(
        &mut self,
        fec_header: FecHeader,
        packet: &[u8],
    ) -> Option<Vec<Vec<u8>>> {

        // Check if we've already successfully decoded this block
        if self.decoded_blocks.contains(&fec_header.block_id) {
            // Already decoded this block, ignore this packet
            return None;
        }

        // Get or create decoder for this block
        if !self.fec_decoders.contains_key(&fec_header.block_id) {
            // Create ObjectTransmissionInformation with proper parameters
            // (transfer_length, symbol_size, sub_symbol_size, source_symbols, repair_symbols)

            let oti = get_raptorq_oti(fec_header.block_size, fec_header.packet_size);
            self.fec_decoders
                .insert(fec_header.block_id, Decoder::new(oti));
        }

        let decoder = self.fec_decoders.get_mut(&fec_header.block_id).unwrap();
        
        let packet = EncodingPacket::deserialize(packet);

        // add packet to decoder
        // Try to decode with current packets
        if let Some(mut decoded_data) = decoder.decode(packet) {
            // Successfully decoded! Get the original udp packages:
            let Some(num_pkgs) = decoded_data.pop() else { return None};
            if decoded_data.len() < num_pkgs as usize * 2 { return None };
            let indices_start_index = decoded_data.len() - num_pkgs as usize * 2;
            let pkg_indices: Vec<_> = decoded_data[indices_start_index..]
                .chunks(2)
                .map(|b| u16::from_le_bytes(b.try_into().unwrap()))
                .chain(once(indices_start_index as u16))
                .collect();
            let mut packets = Vec::new();
            for i in pkg_indices.windows(2) {
                let (start, end) = (i[0] as usize, i[1] as usize);
                packets.push(decoded_data[start..end].to_vec());
            }

            // Clean up
            self.fec_decoders.remove(&fec_header.block_id);
            self.decoded_blocks.insert(fec_header.block_id);

            return Some(packets);
        }

        // Clean up old decoders to prevent memory leak
        // Remove decoders older than current block_id - 100
        let cleanup_limit = 64;
        let cleanup_threshold_high = fec_header.block_id.wrapping_add(cleanup_limit);
        let cleanup_threshold_low = fec_header.block_id.wrapping_sub(cleanup_limit);
        let condition: Box<dyn Fn(u8) -> bool> = if cleanup_threshold_high > cleanup_threshold_low {
            Box::new(|a| cleanup_threshold_low < a && a < cleanup_threshold_high)
        } else {
            Box::new(|a| cleanup_threshold_low < a || a < cleanup_threshold_high)
        };
        self.fec_decoders.retain(|&k, _| condition(k));
        // Also clean up decoded blocks tracker
        self.decoded_blocks.retain(|&k| condition(k));

        return None; // Need more packets
    }

    // Reads and removes the radiotap and wifi headers
    fn process_packet(
        &self,
        packet: &Packet,
    ) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error>> {
        let data = packet.data;

        if data.len() < 4 {
            eprintln!("packet too short");
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
            eprintln!("packet has no payload");
            return Ok(None); // No payload
        }

        let payload = &data[payload_start..];

        //there are four bytes at the end where i dont know where it is coming from, so i remove them
        //TODO figure out what that is
        let payload = &payload[..payload.len().saturating_sub(4)];

        if payload.len() > self.buffer_size {
            eprintln!("payload too large / buffer size too small");
            return Ok(None); // Payload too large
        }

        Ok(Some(payload.to_vec()))
    }

    fn open_wifi_capture(&self, wifi_device: String) -> Result<Capture<Active>, Box<dyn std::error::Error>> {
        let wifi_max_size = 4096;

        let wifi_card = pcap::Device::list()?
            .into_iter()
            .find(|dev| dev.name == wifi_device)
            .ok_or_else(|| format!("WiFi device {} not found", wifi_device))?;

        let mut cap = pcap::Capture::from_device(wifi_card)?
            .snaplen(wifi_max_size)
            .promisc(true)
            .timeout(100) // 100ms timeout instead of -1
            .immediate_mode(true)
            .open()?;

        if cap.get_datalink() != pcap::Linktype::IEEE802_11_RADIOTAP {
            return Err(format!("Unknown encapsulation on interface {}", wifi_device).into());
        }

        // Set the BPF filter to match the original C++ code
        let filter = format!(
            "ether[0x0a:2]==0x5742 && ether[0x0c:4] == {:#010x}",
            self.channel_id
        );
        cap.filter(&filter, true)?;

        cap = cap.setnonblock()?;

        Ok(cap)
    }
}
