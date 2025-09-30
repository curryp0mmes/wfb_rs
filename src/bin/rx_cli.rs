use clap::Parser;
use std::time::Duration;
use wfb_rs::common;
#[cfg(feature = "receiver")]
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
    #[arg(short = 'i', long, default_value_t = 7669206)]
    link_id: u32,

    /// Receiving Buffer Size
    #[arg(short = 'R', long, default_value_t = 0)]
    buffer_size: usize,

    /// Log Interval
    #[arg(short='l', long, default_value = "1000", value_parser = parse_duration)]
    log_interval: Duration,

    /// Wifi Card setup (channel 149, monitor mode)
    #[arg(long, default_value_t = false)]
    wifi_setup: bool,

    /// Key File Location (unused)
    #[arg(short = 'K', long, default_value = "")]
    key_file: String,

    /// Wifi Device
    wifi_device: String,
    // TODO add args other modes?
}

fn parse_duration(arg: &str) -> Result<std::time::Duration, std::num::ParseIntError> {
    let milliseconds = arg.parse()?;
    Ok(std::time::Duration::from_millis(milliseconds))
}

#[cfg(feature = "receiver")]
fn main() {
    let args = Args::parse();

    println!("{:?}", args);

    if args.wifi_setup {
        common::set_monitor_mode(args.wifi_device.as_str()).unwrap();
    }

    let mut _rx = Receiver::new(
        args.client_address,
        args.client_port,
        args.radio_port,
        args.link_id,
        args.buffer_size,
        args.log_interval,
        args.wifi_device,
    );

    let _ = _rx.run().unwrap();
}

#[cfg(not(feature = "receiver"))]
fn main() {
    println!("Receiver was not built, recompile with --features=receiver")
}
