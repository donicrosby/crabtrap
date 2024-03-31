use crate::{tarpit_impl, CrabTrapConfig, TarPit, TarpitWriter};
use hyper::service;
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto;
use std::io;
use thiserror::Error;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tower::ServiceBuilder;
use tracing::{error, info};

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

        let (min_body, max_body, content_type) = (
            self.config.tarpit_config().min_body_size(),
            self.config.tarpit_config().max_body_size(),
            self.config.tarpit_config().content_type(),
        );
        let (conn_send, conn_recv) = mpsc::unbounded_channel();
        let mut writer = TarpitWriter::new(
            self.config.tarpit_config().tick_duration(),
            self.config.tarpit_config().duration_per_byte(),
            conn_recv,
        );
        let _writer_handle = tokio::task::spawn(async move { writer.process_connections().await });

        loop {
            let (stream, rmt_addr) = listener.accept().await?;
            info!("Received connection from: {}", rmt_addr);
            let conn_send = conn_send.clone();
            let content_type = content_type.clone();
            let io = TokioIo::new(stream);
            tokio::task::spawn(async move {
                let svc = service::service_fn(tarpit_impl);
                let svc = ServiceBuilder::new()
                    .layer_fn(|svc| {
                        TarPit::new(
                            min_body,
                            max_body,
                            content_type.clone(),
                            conn_send.clone(),
                            svc,
                        )
                    })
                    .service(svc);
                if let Err(err) = auto::Builder::new(TokioExecutor::new())
                    .serve_connection(io, svc)
                    .await
                {
                    error!("Error serving connection: {:?}", err);
                }
            });
        }
    }
}
