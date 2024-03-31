use hyper::body::Bytes;
use thiserror::Error;
use tokio::sync::mpsc;

mod connection;
mod service;

pub(crate) use self::connection::{
    ClientMetadata, ConnectionMetadata, TarpitConnection,
};
pub use self::service::ContentType;
pub(crate) use self::service::{tarpit_impl, TarPit, TarPitMetadataCollector};

pub(crate) type TarpitSender = mpsc::UnboundedSender<Bytes>;
pub(crate) type TarpitRecv = mpsc::UnboundedReceiver<Bytes>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("hyper error")]
    Hyper(#[from] hyper::Error),

    #[error("http error")]
    Http(#[from] hyper::http::Error),

    #[error("channel error")]
    Channel(#[from] mpsc::error::SendError<Bytes>),

    #[error("content type parse")]
    ContentTypeParse(#[from] mime::FromStrError),
}
