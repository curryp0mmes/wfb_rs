use clap::Parser;

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
    #[arg(short, long, default_value_t = 0)]
    buffer_size: usize,

    /// FEC delay
    #[arg(short, long, default_value_t = 0)]
    fec_delay: u32,

    /// Bandwidth
    #[arg(short, long, default_value_t = 20)]
    bandwidth: u32,

    /// Short GI
    #[arg(short, long, default_value_t = false)]
    short_gi: bool,

    /// STBC
    #[arg(short, long, default_value_t = 1)]
    stbc: u32,

    /// LDPC
    #[arg(short, long, default_value_t = 1)]
    ldpc: u32,

    /// MCS Index
    #[arg(short, long, default_value_t = 9)]
    mcs_index: u32,

    /// vht nss
    #[arg(short, long, default_value_t = 1)]
    vht_nss: u32,

    /// Debug Port
    #[arg(short, long, default_value_t = 0)]
    debug_port: u16,

    /// FEC Timeout
    #[arg(short, long, default_value_t = 1000)]
    fec_timeout: u64,

    /// Log Interval
    #[arg(short, long, default_value_t = 1000)]
    log_interval: u64,

    /// Link ID
    #[arg(short, long, default_value_t = 0)]
    link_id: u32,

    /// Epoch
    #[arg(short, long, default_value_t = 0)]
    epoch: u64,

    /// Mirror mode
    #[arg(short, long, default_value_t = false)]
    mirror: bool,

    /// VHT Mode
    #[arg(short, long, default_value_t = 0)]
    vht_mode: u32,

    /// Control Port
    #[arg(short, long, default_value_t = 9000)]
    control_port: u16,

    // TODO args frametype, qdisc, fwmark, other modes?
}

fn main() {
    let args = Args::parse();

    println!("{:?}", args);
}
