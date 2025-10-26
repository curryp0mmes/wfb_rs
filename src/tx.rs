mod tx_hardware_interface;
mod tx_fec;

use std::net::UdpSocket;
use std::sync::mpsc::channel;
use std::time::Duration;
use std::{io, thread};

use super::common::{hw_headers, bandwidth::Bandwidth};

use tx_hardware_interface::TXHwInt;
use tx_fec::TXFec;

pub struct Transmitter {
    tx: TXHwInt,
    fec: Option<TXFec>,
}

impl Transmitter {
    pub fn new(
        magic: u32,
        radio_port: u8,
        link_id: u32,
        bandwidth: Bandwidth,
        short_gi: bool,
        stbc: u8,
        ldpc: bool,
        mcs_index: u8,
        vht_mode: bool,
        vht_nss: u8,
        wifi_device: String,
        fec_disabled: bool,
        min_block_size: u16,
        wifi_packet_size: u16,
        redundant_pkgs: u32,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let radiotap_header = hw_headers::get_radiotap_headers(
            stbc, ldpc, short_gi, bandwidth, mcs_index, vht_mode, vht_nss,
        );
        let link_id = link_id & 0xffffff;

        let channel_id = link_id << 8 | radio_port as u32;

        let tx = TXHwInt::new(wifi_device, radiotap_header, channel_id)?;

        let fec = if fec_disabled {
            None
        } else { Some(TXFec::new(
            magic,
            min_block_size,
            wifi_packet_size,
            redundant_pkgs
        ))};

        Ok(Self {
            tx,
            fec
        })
    }

    pub fn run(mut self, source_port: u16, buffer_r: usize, log_interval: Duration) -> Result<(), Box<dyn std::error::Error>> {

        let udp_socket = UdpSocket::bind(format!("0.0.0.0:{}", source_port))?;
        
        let (sent_bytes_s, sent_bytes_r) = channel();
        let (received_bytes_s, received_bytes_r) = channel();

        // start logtask
        thread::spawn(move || {
            loop {
                let (sent_packets, sent_bytes): (u32, u32) = sent_bytes_r.try_iter().fold((0, 0), |(count, sum), v| (count + 1, sum + v));
                let (received_packets, received_bytes): (u32, u32) = received_bytes_r.try_iter().fold((0, 0), |(count, sum), v| (count + 1, sum + v));
                println!(
                    "Packets R->T {}->{},\tBytes {}->{}",
                    sent_packets,
                    received_packets,
                    received_bytes,
                    sent_bytes,
                );
                thread::sleep(log_interval);
            }
        });

        let (block_s, block_r) = channel::<Vec<Vec<u8>>>();
        
        thread::spawn(move || {
            for block in block_r.into_iter() {
                for packet in block.into_iter() {
                    let sent = self.tx.send_packet(&packet).unwrap() as u32;
                    if sent < packet.len() as u32 {
                        eprintln!("socket dropped some bytes");
                    }
                    sent_bytes_s.send(sent as u32).unwrap();
                }
            }
        });

        loop {
            let mut udp_recv_buffer = vec![0u8; buffer_r];
            let poll_result = udp_socket.recv(&mut udp_recv_buffer);

            match poll_result {
                Err(err) => match err.kind() {
                    io::ErrorKind::TimedOut => continue,
                    err => {
                        eprintln!("Error polling udp input: {}", err);
                        continue;
                    },
                },
                Ok(received) => {
                    if received == 0 {
                        //Empty packet
                        eprintln!("Empty packet");
                        continue;
                    }
                    if received == buffer_r {
                        eprintln!("Input packet seems too large");
                    }
                    
                    let udp_packet = &udp_recv_buffer[..received];

                    received_bytes_s.send(received as u32)?;

                    if let Some(fec) = self.fec.as_mut() {
                        if let Some(block) = fec.process_packet_fec(udp_packet) {
                            block_s.send(block)?;
                        }
                    } else {
                        // if fec is disabled just immediately return the raw block
                        block_s.send(vec![udp_packet.to_vec()])?;
                    }
                }
            }
            
        }
    }
}
