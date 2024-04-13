use crate::{
    handle_tarpit_connection, CrabTrapConfig, HostExtractorLayer, RequestInfoExtractorLayer,
    Tarpit, TarpitMetadataCollectorLayer, UserAgentExtractorLayer,
};
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto;
use hyper_util::service::TowerToHyperService;
use std::io;
use thiserror::Error;
use tokio::net::TcpListener;
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
        let connection_svc = ServiceBuilder::new().service_fn(handle_tarpit_connection);
        let mut tarpit = Tarpit::new(connection_svc, self.config.tarpit_config().clone());
        let tarpit_worker = tarpit.clone();

        let tarpit_handle =
            tokio::task::spawn(async move { tarpit_worker.process_connections().await });
        tarpit.add_handle(tarpit_handle).await;

        loop {
            let (stream, rmt_addr) = listener.accept().await?;
            info!("Received connection from: {}", rmt_addr);
            let io = TokioIo::new(stream);
            let request_tarpit = tarpit.clone();
            tokio::task::spawn(async move {
                let svc = ServiceBuilder::new()
                    .layer(HostExtractorLayer)
                    .layer(UserAgentExtractorLayer)
                    .layer(RequestInfoExtractorLayer)
                    .layer(TarpitMetadataCollectorLayer)
                    .service(request_tarpit);
                let svc = TowerToHyperService::new(svc);
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
