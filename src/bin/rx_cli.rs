use clap::Parser;
use wfb_rs::Receiver;

/// Receiving side of wfb_rs
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// FEC k
    #[arg(short, default_value_t = 8)]
    k: u32,

    /// FEC n
    #[arg(short, default_value_t = 12)]
    n: u32,

    /// Forwarding Address
    #[arg(short = 'c', long, default_value = "127.0.0.1")]
    client_address: String,

    /// Forwarding Port
    #[arg(short = 'u', long, default_value_t = 5600)]
    client_port: u16,

    /// Listening Port
    #[arg(short = 'p', long, default_value_t = 0)]
    radio_port: u16,

    /// Link ID
    #[arg(short = 'l', long, default_value_t = 0)]
    link_id: u32,

    /// Receiving Buffer Size
    #[arg(short, long, default_value_t = 0)]
    buffer_size: usize,

    /// Log Interval
    #[arg(short='I', long, default_value_t = 1000)]
    log_interval: u64,

    /// Wifi Device
    wifi_device: String,
    // TODO add args other modes?
}

fn main() {
    let args = Args::parse();

    let _rx = Receiver::new(
        args.client_address,
        args.client_port,
        args.radio_port,
        args.link_id,
        args.buffer_size,
        args.log_interval,
        args.wifi_device,
    );
}
