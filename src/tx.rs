use std::{fmt::format, net::UdpSocket, vec};

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
        //Bind to all wifi devices

        println!("Binding {} to Port {}", self.wifi_device, self.udp_port);
        let udp_receiver = UdpSocket::bind(format!("0.0.0.0:{}", self.udp_port))
            .expect("Failed to bind to udp port");

        assert!(self.buffer_size > 0);
        let mut buffer: Vec<u8> = vec![0; self.buffer_size];
        //TODO own thread for the udp socket polling
        loop {
            match udp_receiver.recv(&mut buffer) {
                Ok(datasize) => {
                    println!("Received data with length {}: {:?}", datasize, buffer);

                    // TODO header

                    // TODO inject packet into wifistick 

                }
                Err(e) => {
                    println!("Error: {:?}", e);
                }
            }
        }
    }
}
