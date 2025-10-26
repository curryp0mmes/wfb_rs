use clap::Parser;
use std::time::Duration;
#[cfg(feature = "receiver")]
use wfb_rs::{common::utils, Receiver};

/// Receiving side of wfb_rs
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    // Magic number to identify the device
    #[arg(short = 'm', long, default_value_t = 0x57627273)]
    magic: u32,

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

    /// Log Interval
    #[arg(short='l', long, default_value = "1000", value_parser = parse_duration)]
    log_interval: Duration,

    /// Wifi Card setup (channel 149, monitor mode)
    #[arg(short='s', long, default_value_t = false)]
    wifi_setup: bool,

    /// Wifi Device
    #[arg(required = true, num_args = 1..)]
    wifi_devices: Vec<String>
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
        for wifi in &args.wifi_devices {
            utils::set_monitor_mode(wifi.as_str()).unwrap();
        }
    }

    let rx = Receiver::new(
        args.magic,
        args.radio_port,
        args.link_id,
        args.wifi_devices,
    ).unwrap();

    rx.run(
        args.client_address,
        args.client_port,
        args.log_interval
    ).unwrap();
}

#[cfg(not(feature = "receiver"))]
fn main() {
    println!("Receiver was not built, recompile with --features=receiver")
}
