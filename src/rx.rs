use pcap::{self, Active, Capture, Packet};
use radiotap::Radiotap;
use raptorq::{Decoder, EncodingPacket};
use std::collections::HashMap;
use std::net::UdpSocket;
use std::time::{Duration, Instant};

use crate::common;
use crate::common::{FecHeader, FEC_HEADER_SIZE};

pub struct Receiver {
    client_address: String,
    client_port: u16,
    buffer_size: usize,
    log_interval: Duration,
    wifi_device: String,
    channel_id: u32,

    fec_decoders: HashMap<u32, Decoder>,
    fec_packets: HashMap<u32, Vec<Vec<u8>>>,
    decoded_blocks: std::collections::HashSet<u32>, // Track already decoded blocks
    original_lengths: HashMap<u32, u32>,            // Track original data length per block
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

            fec_decoders: HashMap::new(),
            fec_packets: HashMap::new(),
            decoded_blocks: std::collections::HashSet::new(),
            original_lengths: HashMap::new(),
        }
    }

    pub fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let compound_output_address = format!("{}:{}", self.client_address, self.client_port);

        let udp_socket = UdpSocket::bind("0.0.0.0:0")?; // Bind to any available port
        udp_socket.connect(&compound_output_address)?;

        let mut wifi_capture = self.open_wifi_capture()?;

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

            match wifi_capture.next_packet() {
                Ok(packet) if packet.len() > 0 => {
                    received_packets += 1;
                    received_bytes += packet.len() as u64;

                    if let Some(payload) = self.process_packet(&packet)? {
                        if FecHeader::is_fec(&payload) {
                            // Try to parse FEC header
                            if let Some(decoded_data) = self.process_fec_packet(&payload)? {
                                match udp_socket.send(&decoded_data) {
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
        }
    }

    fn process_fec_packet(
        &mut self,
        payload: &[u8],
    ) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error>> {
        // Check if payload has FEC header
        if payload.len() < FEC_HEADER_SIZE {
            // Not a FEC packet, forward as-is
            return Ok(Some(payload.to_vec()));
        }

        // Try to parse FEC header
        if let Some(fec_header) = FecHeader::from_bytes(payload) {
            // Check if we've already successfully decoded this block
            if self.decoded_blocks.contains(&fec_header.block_id) {
                // Already decoded this block, ignore this packet
                return Ok(None);
            }

            // This is a FEC packet
            let fec_packet_data = &payload[FEC_HEADER_SIZE..];

            // Get or create decoder for this block
            if !self.fec_decoders.contains_key(&fec_header.block_id) {
                // Create ObjectTransmissionInformation with proper parameters
                // (transfer_length, symbol_size, sub_symbol_size, source_symbols, repair_symbols)

                use crate::common::get_fec_oti;
                let oti = get_fec_oti();
                self.fec_decoders
                    .insert(fec_header.block_id, Decoder::new(oti));
                self.fec_packets.insert(fec_header.block_id, Vec::new());
                // Store the original length from the first packet of this block
                self.original_lengths
                    .insert(fec_header.block_id, fec_header.original_length);
            }

            let decoder = self.fec_decoders.get_mut(&fec_header.block_id).unwrap();
            let packets = self.fec_packets.get_mut(&fec_header.block_id).unwrap();

            // Store the packet
            packets.push(fec_packet_data.to_vec());

            // Try to decode with current packets
            let encoding_packet = EncodingPacket::deserialize(fec_packet_data);
            if let Some(decoded_data) = decoder.decode(encoding_packet) {
                // Successfully decoded! Get the original length and trim the data
                let original_length = self
                    .original_lengths
                    .get(&fec_header.block_id)
                    .copied()
                    .unwrap_or(decoded_data.len() as u32);
                let trimmed_data = decoded_data
                    [..original_length.min(decoded_data.len() as u32) as usize]
                    .to_vec();

                // Clean up
                self.decoded_blocks.insert(fec_header.block_id);
                self.fec_decoders.remove(&fec_header.block_id);
                self.fec_packets.remove(&fec_header.block_id);
                self.original_lengths.remove(&fec_header.block_id);

                return Ok(Some(trimmed_data));
            }

            // Clean up old decoders to prevent memory leak
            // Remove decoders older than current block_id - 10
            let cleanup_threshold = fec_header.block_id.saturating_sub(10);
            self.fec_decoders.retain(|&k, _| k > cleanup_threshold);
            self.fec_packets.retain(|&k, _| k > cleanup_threshold);
            self.original_lengths.retain(|&k, _| k > cleanup_threshold);
            // Also clean up decoded blocks tracker
            self.decoded_blocks.retain(|&k| k > cleanup_threshold);

            return Ok(None); // Need more packets
        } else {
            // Not a FEC packet - transmit as is
            return Ok(Some(payload.to_vec()));
        }
    }

    // Reads and removes the radiotap and wifi headers
    fn process_packet(
        &self,
        packet: &Packet,
    ) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error>> {
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

        //there are four bytes at the end where i dont know where it is coming from, so i remove them
        //TODO figure out what that is
        let payload = &payload[..payload.len().saturating_sub(4)];

        if payload.len() > self.buffer_size {
            return Ok(None); // Payload too large
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
        let filter = format!(
            "ether[0x0a:2]==0x5742 && ether[0x0c:4] == {:#010x}",
            self.channel_id
        );
        cap.filter(&filter, true)?;

        cap = cap.setnonblock()?;

        Ok(cap)
    }
}
