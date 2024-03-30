use clap::Parser;
use claptrap::{CrabTrapConfig, CrabTrapServer};
use std::error::Error;
use std::net::IpAddr;

/// Crab Trap Tarpit Server
#[derive(Debug, Parser)]
#[command(version, about)]
struct Args {
    /// Host to listen for connections on
    #[arg(short('H'), long, value_parser(clap::value_parser!(IpAddr)))]
    host: IpAddr,

    /// Port to listen on
    #[arg(short, long)]
    port: u16,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    tracing_subscriber::fmt().init();

    let config = CrabTrapConfig::new((args.host, args.port));
    let server = CrabTrapServer::new(config);
    server.run().await?;
    Ok(())
}
