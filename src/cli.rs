use clap::{arg, Parser};

#[derive(Parser, Debug)]
pub struct Args {
    #[arg(long, default_value_t = 6379)]
    pub port: u16,

    #[arg(long)]
    pub replicaof: Option<String>,
}

pub fn parse_cli() -> Args {
    Args::parse()
}
