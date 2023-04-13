use std::{
    net::SocketAddr,
    error::Error,
};

use clap::{Parser, Subcommand, Args};

use crate::config::ConfigServe;


#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
#[clap(propagate_version = true)]
pub struct Cli {
    #[clap(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Serve(Serve),
}


#[derive(Args, Debug)]
pub struct Serve {
    pub address: SocketAddr,
    #[clap(parse(try_from_str = parse_config_main))]
    pub config: ConfigServe,
}

fn parse_config_main(s: &str) -> Result<ConfigServe, Box<dyn Error + Send + Sync + 'static>> {
    let config: ConfigServe = serde_json::from_str::<ConfigServe>( &std::fs::read_to_string(s)? )?;
    Ok(config)
}