use clap::Parser;
use std::time::Duration;
use wfb_rs::common;
#[cfg(feature = "receiver")]
use wfb_rs::Receiver;

/// Receiving side of wfb_rs
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
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
    #[arg(short = 'R', long, default_value_t = 5_000)]
    buffer_size: usize,

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
            common::set_monitor_mode(wifi.as_str()).unwrap();
        }
    }

    let mut _rx = Receiver::new(
        args.client_address,
        args.client_port,
        args.radio_port,
        args.link_id,
        args.buffer_size,
        args.wifi_devices,
    ).unwrap();

    let _ = _rx.run(args.log_interval).unwrap();
}

#[cfg(not(feature = "receiver"))]
fn main() {
    println!("Receiver was not built, recompile with --features=receiver")
}
