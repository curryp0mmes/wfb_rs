use clap::Parser;

/// Receiving side of wfb_rs
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Forwarding Address
    #[arg(short='c', long, default_value = "127.0.0.1")]
    client_address: String,

    /// Forwarding Port
    #[arg(short='u', long, default_value_t = 5600)]
    client_port: u16,

    /// Listening Port
    #[arg(short='p', long, default_value_t = 0)]
    radio_port: u16,

    /// Receiving Buffer Size
    #[arg(short, long, default_value_t = 0)]
    buffer_size: usize,

    /// Log Interval
    #[arg(short, long, default_value_t = 1000)]
    log_interval: u64,

    // TODO add args other modes?
}

fn main() {
    let args = Args::parse();

    println!("{:?}", args);
}
