mod config;
mod metadata;
mod server;
mod tarpit;
mod util;

pub use self::config::{Config as CrabTrapConfig, TarpitConfig};
pub(crate) use self::metadata::{
    ClientMetadata, Error, HostExtractorLayer, RequestInfoExtractorLayer,
    TarpitMetadataCollectorLayer, UserAgentExtractorLayer,
};
pub use self::server::Server as CrabTrapServer;
pub(crate) use self::tarpit::Tarpit;
pub(crate) use self::util::extract_header;
pub use self::util::ContentType;

#[cfg(test)]
pub(crate) use self::metadata::{HostMetadata, RequestInfoMetadata, UserAgentMetadata};
