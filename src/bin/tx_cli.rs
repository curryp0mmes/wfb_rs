use std::time::Duration;

use clap::Parser;
use wfb_rs::Transmitter;

use wfb_rs::common::Bandwidth;

/// Receiving side of wfb_rs
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// FEC k
    #[arg(short='k', long, default_value_t = 8)]
    k: u32,

    /// FEC n
    #[arg(short='n', long, default_value_t = 12)]
    n: u32,

    /// Sending Radio Port
    #[arg(short='p', long, default_value_t = 0)]
    radio_port: u8,

    /// Data Input Port
    #[arg(short='u', long, default_value_t = 5600)]
    udp_port: u16,

    /// Receiving Buffer Size
    #[arg(short='R', long, default_value_t = 1024)]
    buffer_size_recv: usize,

    /// Sending Buffer Size
    #[arg(short='s', long, default_value_t = 1024)]
    buffer_size_send: usize,

    /// FEC delay
    #[arg(short='F', long, default_value_t = 0)]
    fec_delay: u32,

    /// Bandwidth
    #[arg(short='B', long, default_value = "bw20", value_parser = clap::value_parser!(Bandwidth))]
    bandwidth: Bandwidth,

    /// Short GI
    #[arg(short = 'G', long, default_value_t = false)]
    short_gi: bool,

    /// STBC
    #[arg(short='S', long, default_value_t = 1)]
    stbc: u8,

    /// LDPC
    #[arg(short='L', long, default_value_t = true)]
    ldpc: bool,

    /// MCS Index
    #[arg(short='M', long, default_value_t = 1)] //TODO why was the default 9?
    mcs_index: u8,

    /// vht nss
    #[arg(short='N', long, default_value_t = 1)]
    vht_nss: u8,

    /// Debug Port
    #[arg(short='D', long, default_value_t = 0)]
    debug_port: u16,

    /// FEC Timeout
    #[arg(short = 'T', long, default_value_t = 1000)]
    fec_timeout: u64,

    /// Log Interval
    #[arg(short='l', long, default_value = "1000", value_parser = parse_duration)]
    log_interval: Duration,

    /// Link ID
    #[arg(short = 'i', long, default_value_t = 0)]
    link_id: u32,

    /// Epoch
    #[arg(short, long, default_value_t = 0)]
    epoch: u64,

    /// Mirror mode
    #[arg(short = 'm', long, default_value_t = false)]
    mirror: bool,

    /// VHT Mode
    #[arg(short = 't', long, default_value_t = false)]
    vht_mode: bool,

    /// Control Port
    #[arg(short, long, default_value_t = 9000)]
    control_port: u16,

    /// Key File Location (unused, just here for compatibility)
    #[arg(short='K', long, default_value = "")]
    key_file: String,

    /// Wifi Devices
    wifi_device: String,
    // TODO args frametype, qdisc, fwmark, other modes?
}

fn parse_duration(arg: &str) -> Result<std::time::Duration, std::num::ParseIntError> {
    let milliseconds = arg.parse()?;
    Ok(std::time::Duration::from_millis(milliseconds))
}

fn main() {
    let args = Args::parse();

    println!("{:?}", args);

    let mut tx = Transmitter::new(
        args.radio_port,
        args.link_id,
        args.buffer_size_recv,
        args.buffer_size_send,
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

    let _ = tx.run().unwrap();
}
