use crate::{hello, CrabTrapConfig};
use hyper::service::service_fn;
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto;
use std::io;
use thiserror::Error;
use tokio::net::TcpListener;
use tracing::{info, error};

#[derive(Debug, Error)]
pub enum Error {
    #[error("socket error")]
    Socket(#[from] io::Error),
}

pub struct Server {
    config: CrabTrapConfig,
}

impl Server {
    pub fn new(config: CrabTrapConfig) -> Self {
        Self { config }
    }

    pub async fn run(&self) -> Result<(), Error> {
        info!("Starting Crab Trap Tarpit...");
        let listener = TcpListener::bind(self.config.bind_addr()).await?;
        info!("Listening on: {}", self.config.bind_addr());

        loop {
            let (stream, rmt_addr) = listener.accept().await?;
            info!("Received connection from: {}", rmt_addr);

            let io = TokioIo::new(stream);

            tokio::task::spawn(async move {
                if let Err(err) = auto::Builder::new(TokioExecutor::new())
                    .serve_connection(io, service_fn(hello))
                    .await
                {
                    error!("Error serving connection: {:?}", err);
                }
            });
        }
    }
}
