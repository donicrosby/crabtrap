use hyper::body::Bytes;
use thiserror::Error;
use tokio::sync::mpsc;

mod collector;
mod host;
mod request_info;
mod user_agent;

static NO_METADATA_FOUND: &str = "N/A";

pub(crate) use self::collector::{ClientMetadata, TarpitMetadataCollectorLayer};
pub(crate) use self::host::{HostExtractorLayer, HostMetadata};
pub(crate) use self::request_info::{RequestInfoExtractorLayer, RequestInfoMetadata};
pub(crate) use self::user_agent::{UserAgentExtractorLayer, UserAgentMetadata};

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
