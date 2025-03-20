pub struct Receiver {
    client_address: String,
    client_port: u16,
    radio_port: u16,
    buffer_size: usize,
    log_interval: u64,
    wifi_device: String,
}

impl Receiver {
    pub fn new(
        client_address: String,
        client_port: u16,
        radio_port: u16,
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
        }
    }

    pub fn run(&self) {
        loop {

        }
    }
}
