use async_trait::async_trait;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

pub use self::tarpit::TarpitWriter;
use crate::{ConnectionMetadata, TarpitConnection};

mod mock;
mod tarpit;

/// Unbounded Receiver for Writer to receive new connections on
pub(crate) type TarpitConnRecv = UnboundedReceiver<TarpitConnection>;

/// Unbounded Sender for Server to send new connections to Writer
pub(crate) type TarpitConnSend = UnboundedSender<TarpitConnection>;

#[async_trait]
pub trait StreamingBytesWriter {
    async fn process_connections(&mut self, mut conn_recv: TarpitConnRecv);
    async fn get_connections(&self) -> Vec<ConnectionMetadata>;
}
