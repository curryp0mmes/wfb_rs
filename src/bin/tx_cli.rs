use std::time::Duration;

use clap::Parser;
use wfb_rs::{common, Transmitter};

use wfb_rs::common::Bandwidth;

/// Receiving side of wfb_rs
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Explicitly disable fec
    #[arg(short = 'f', long, default_value_t = false)]
    fec_disabled: bool,

    /// Sending Radio Port
    #[arg(short = 'p', long, default_value_t = 0)]
    radio_port: u8,

    /// Data Input Port
    #[arg(short = 'u', long, default_value_t = 5600)]
    udp_port: u16,

    /// Receiving Buffer Size
    #[arg(short = 'R', long, default_value_t = 5_000)]
    buffer_size: usize,

    // (max) Size of each package send over wifi (needs to match with rx)
    #[arg(short = 'W', long, default_value_t = 800)]
    wifi_packet_size: u16,

    // (min) Size of each fec block
    #[arg(short = 'B', long, default_value_t = 10_000)]
    block_size: u16,

    // Number of redundant packages send per block
    #[arg(short = 'r', long, default_value_t = 15)]
    redundant_pkgs: u32,

    /// Bandwidth
    #[arg(short='b', long, default_value = "20", value_parser = parse_bandwidth)]
    bandwidth: Bandwidth,

    /// Short GI
    #[arg(short = 'G', long, action=clap::ArgAction::SetFalse, default_value_t = true)]
    short_gi: bool,

    /// STBC
    #[arg(short = 'S', long, default_value_t = 1)]
    stbc: u8,

    /// LDPC
    #[arg(short = 'L', long, default_value_t = true)]
    ldpc: bool,

    /// MCS Index
    #[arg(short = 'M', long, default_value_t = 1)] //TODO why was the default 9?
    mcs_index: u8,

    /// vht nss
    #[arg(short = 'N', long, default_value_t = 1)]
    vht_nss: u8,

    /// Log Interval
    #[arg(short='l', long, default_value = "1000", value_parser = parse_duration)]
    log_interval: Duration,

    /// Link ID
    #[arg(short = 'i', long, default_value_t = 7669206)]
    link_id: u32,

    /// Epoch
    #[arg(long, default_value_t = 0)]
    epoch: u64,

    /// VHT Mode
    #[arg(long, default_value_t = false)]
    vht_mode: bool,

    /// Control Port
    #[arg(short = 'C', long, default_value_t = 9000)]
    control_port: u16,

    /// Wifi Card setup (channel 149, monitor mode)
    #[arg(short = 's', long, default_value_t = false)]
    wifi_setup: bool,

    /// Tx Power Index (0-64)
    #[arg(short = 't', long)]
    txpower: Option<u8>,

    /// Wifi Devices
    wifi_device: String,
    // TODO args frametype, qdisc, fwmark, other modes?
}

fn parse_duration(arg: &str) -> Result<std::time::Duration, std::num::ParseIntError> {
    let milliseconds = arg.parse()?;
    Ok(std::time::Duration::from_millis(milliseconds))
}

fn parse_bandwidth(arg: &str) -> Result<Bandwidth, String> {
    match arg {
        "10" => Ok(Bandwidth::Bw10),
        "20" => Ok(Bandwidth::Bw20),
        "40" => Ok(Bandwidth::Bw40),
        "80" => Ok(Bandwidth::Bw80),
        "160" => Ok(Bandwidth::Bw160),
        _ => Err("Invalid Bandwidth!".to_string()),
    }
}

fn main() {
    let args = Args::parse();

    println!("{:?}", args);

    if args.wifi_setup {
        let _ = common::set_monitor_mode(args.wifi_device.as_str()).unwrap();
    }
    if let Some(tx_power) = args.txpower {
        let _ = common::set_tx_power(args.wifi_device.as_str(), tx_power).unwrap();
    }

    let tx = Transmitter::new(
        args.radio_port,
        args.link_id,
        args.buffer_size,
        args.udp_port,
        args.bandwidth,
        args.short_gi,
        args.stbc,
        args.ldpc,
        args.mcs_index,
        args.vht_mode,
        args.vht_nss,
        args.wifi_device,
        args.fec_disabled,
        args.block_size,
        args.wifi_packet_size,
        args.redundant_pkgs
    ).unwrap();

    let _ = tx.run(args.log_interval).unwrap();
}
