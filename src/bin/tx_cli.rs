use std::time::Duration;

use clap::Parser;
use wfb_rs::Transmitter;

use wfb_rs::common::{Bandwidth, Bandwidth::*};

/// Receiving side of wfb_rs
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// k
    #[arg(short, long, default_value_t = 8)]
    k: u32,

    /// n
    #[arg(short, long, default_value_t = 12)]
    n: u32,

    /// Sending Radio Port
    #[arg(short, long, default_value_t = 0)]
    radio_port: u16,

    /// Data Input Port
    #[arg(short, long, default_value_t = 5600)]
    udp_port: u16,

    /// Receiving Buffer Size
    #[arg(short, long, default_value_t = 1024)]
    buffer_size: usize,

    /// FEC delay
    #[arg(short, long, default_value_t = 0)]
    fec_delay: u32,

    /// Bandwidth
    #[arg(short='B', long, default_value_t = Bw20, value_parser = clap::value_parser!(Bandwidth))]
    bandwidth: Bandwidth,

    /// Short GI
    #[arg(short='G', long, default_value_t = false)]
    short_gi: bool,

    /// STBC
    #[arg(short, long, default_value_t = 1)]
    stbc: u8,

    /// LDPC
    #[arg(short, long, default_value_t = true)]
    ldpc: bool,

    /// MCS Index
    #[arg(short, long, default_value_t = 9)]
    mcs_index: u8,

    /// vht nss
    #[arg(short, long, default_value_t = 1)]
    vht_nss: u8,

    /// Debug Port
    #[arg(short, long, default_value_t = 0)]
    debug_port: u16,

    /// FEC Timeout
    #[arg(short='F', long, default_value_t = 1000)]
    fec_timeout: u64,

    /// Log Interval
    #[arg(short='I', long, default_value = "1000", value_parser = parse_duration)]
    log_interval: Duration,

    /// Link ID
    #[arg(short='i', long, default_value_t = 0)]
    link_id: u32,

    /// Epoch
    #[arg(short, long, default_value_t = 0)]
    epoch: u64,

    /// Mirror mode
    #[arg(short='M', long, default_value_t = false)]
    mirror: bool,

    /// VHT Mode
    #[arg(short='t', long, default_value_t = false)]
    vht_mode: bool,

    /// Control Port
    #[arg(short, long, default_value_t = 9000)]
    control_port: u16,

    /// Wifi Devices
    wifi_device: String,

    // TODO args frametype, qdisc, fwmark, other modes?
}

fn parse_duration(arg: &str) -> Result<std::time::Duration, std::num::ParseIntError> {
    let seconds = arg.parse()?;
    Ok(std::time::Duration::from_secs(seconds))
}

fn main() {
    let args = Args::parse();

    println!("{:?}", args);

    let mut tx = Transmitter::new(
        args.radio_port,
        args.buffer_size,
        args.log_interval,
        args.k,
        args.n,
        args.udp_port,
        args.fec_delay,
        args.bandwidth,
        args.short_gi,
        args.stbc,
        args.ldpc,
        args.mcs_index,
        args.vht_mode,
        args.vht_nss,
        args.debug_port,
        args.fec_timeout,
        args.wifi_device,
    );

    tx.run();
}
