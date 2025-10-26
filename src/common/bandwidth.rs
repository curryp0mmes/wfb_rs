use clap::ValueEnum;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Bandwidth {
    Bw10,
    Bw20,
    Bw40,
    Bw80,
    Bw160,
}

impl ToString for Bandwidth {
    fn to_string(&self) -> String {
        match self {
            Bandwidth::Bw10 => "10".to_string(),
            Bandwidth::Bw20 => "20".to_string(),
            Bandwidth::Bw40 => "40".to_string(),
            Bandwidth::Bw80 => "80".to_string(),
            Bandwidth::Bw160 => "160".to_string(),
        }
    }
}
