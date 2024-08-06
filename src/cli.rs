use clap::{arg, Parser};
use std::str::FromStr;

#[derive(Debug, Clone)]
pub struct ReplicaInfo {
    pub host: String,
    pub port: u16,
}

impl FromStr for ReplicaInfo {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (host, port_str) = s.split_once(' ').ok_or("Invalid format. Use host port")?;

        let port = port_str.parse::<u16>().map_err(|_| "Invalid port number")?;

        Ok(ReplicaInfo {
            host: host.to_string(),
            port,
        })
    }
}

#[derive(Parser, Debug)]
pub struct Args {
    #[arg(long, default_value_t = 6379)]
    pub port: u16,

    #[arg(long)]
    pub replicaof: Option<ReplicaInfo>,

    #[arg(long, default_value = "")]
    pub dir: String,

    #[arg(long, default_value = "")]
    pub dbfilename: String,
}

pub fn parse_cli() -> Args {
    Args::parse()
}
