use clap::Parser;
use crabtrap::{ContentType, CrabTrapConfig, CrabTrapServer, TarpitConfig};
use std::error::Error;
use std::net::IpAddr;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Crab Trap Tarpit Server
#[derive(Debug, Parser)]
#[command(version, about)]
struct Args {
    /// Host to listen for connections on
    #[arg(short('H'), long, value_parser(clap::value_parser!(IpAddr)), default_value("0.0.0.0"))]
    host: IpAddr,

    /// Port to listen on
    #[arg(short, long, default_value("3000"))]
    port: u16,

    /// Minimum amount of bytes to send as the body
    #[arg(short('m'), long, default_value("1048576"))]
    min_body_size: u64,

    /// Maximum amount of bytes to send as the body
    #[arg(short('M'), long, default_value("10485760"))]
    max_body_size: u64,

    /// Number of milliseconds to sleep before processing some connections
    #[arg(short, long, default_value("50"))]
    tick_duration: u64,

    /// Number of seconds between sending a character to a tarpit connection
    #[arg(short, long, default_value("16"))]
    duration_per_byte: u64,

    /// Content Type to send payloads as
    #[arg(short, long, default_value("text/plain"))]
    content_type: ContentType,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    let config = CrabTrapConfig::new(
        (args.host, args.port),
        TarpitConfig::new(
            args.min_body_size,
            args.max_body_size,
            args.tick_duration,
            args.duration_per_byte,
            args.content_type,
        ),
    );
    let server = CrabTrapServer::new(config);

    server.run().await?;
    Ok(())
}
