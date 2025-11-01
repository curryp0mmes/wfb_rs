mod rx_hardware_interface;
mod rx_fec;

use std::net::UdpSocket;
use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;

use rx_hardware_interface::RXHwInt;
use rx_fec::RXFec;
use crate::common::magic_header::MagicHeader;

pub struct Receiver {
    rxs: Vec<RXHwInt>,
    fec: RXFec,
    magic_header: MagicHeader,
}

impl Receiver {
    pub fn new(
        magic: u32,
        radio_port: u16,
        link_id: u32,
        wifi_devices: Vec<String>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let channel_id = link_id << 8 | radio_port as u32;

        let rxs: Vec<RXHwInt> = wifi_devices
            .into_iter()
            .map(|dev| RXHwInt::new(dev, channel_id))
            .collect::<Result<_, _>>()?;


        let fec = RXFec::new();

        let magic_header = MagicHeader::new(magic);

        Ok(Self {
            rxs,
            fec,
            magic_header,
        })
    }

    pub fn run(mut self,
        client_address: String,
        client_port: u16,
        log_interval: Duration)
        -> Result<(), Box<dyn std::error::Error>> {

        let udp_socket = UdpSocket::bind("0.0.0.0:0")?; // Bind to any available port
        
        let compound_output_address = format!("{}:{}", client_address, client_port);
        udp_socket.connect(&compound_output_address)?;
        
        let (sent_bytes_s, sent_bytes_r) = channel();
        let (received_bytes_s, received_bytes_r) = channel();

        // start logtask
        thread::spawn(move || {
            loop {
                let (sent_packets, sent_bytes): (u32, u32) = sent_bytes_r.try_iter().fold((0, 0), |(count, sum), v| (count + 1, sum + v));
                let (received_packets, received_bytes): (u32, u32) = received_bytes_r.try_iter().fold((0, 0), |(count, sum), v| (count + 1, sum + v));
                println!(
                    "Packets R->T {}->{},\tBytes {}->{}",
                    received_packets,
                    sent_packets,
                    received_bytes,
                    sent_bytes,
                );
                thread::sleep(log_interval);
            }
        });

        loop {
            for rx in &mut self.rxs {
                let Some(raw_packet) = rx.receive_packet()? else { continue; };
                received_bytes_s.send(raw_packet.len() as u32)?;

                let Some((fec_pkg, wfb_packet)) = self.magic_header.from_bytes(&raw_packet) else { continue; };
                
                let decoded_data = if fec_pkg {
                    let Some(decoded_data) = self.fec.process_fec_packet(&wfb_packet) else { continue; };
                    decoded_data
                } else {
                    vec![wfb_packet.to_vec()]
                };

                for udp_pkg in decoded_data {
                    match udp_socket.send(&udp_pkg) {
                        Err(e) => {
                            eprintln!("Error forwarding packet: {}", e);
                        }
                        Ok(sent) => {
                            sent_bytes_s.send(sent as u32)?;
                        }
                    }
                }
            }
        }
    }
}
