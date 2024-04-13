use super::ClientMetadata;
use crate::{ContentType, Error, TarpitRecv, TarpitSender};
use getset::{CopyGetters, Getters, MutGetters, Setters};
use hyper::body::Bytes;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, Getters, Setters, CopyGetters)]
pub struct RequestMetadata {
    /// Number of bytes sent to the client
    #[getset(get_copy = "pub")]
    bytes_sent: u64,
    /// Total number of random bytes to send to the client
    #[getset(get_copy = "pub")]
    response_size: u64,
    /// Instant the last byte was sent to the client
    #[getset(get = "pub", set = "pub")]
    time_since_last_byte: Option<Instant>,
}

impl RequestMetadata {
    /// Create a new RequestMetadata object
    pub fn new(response_size: u64) -> Self {
        Self {
            bytes_sent: 0,
            time_since_last_byte: None,
            response_size,
        }
    }

    /// Increment the number of bytes sent by one
    /// Used to help determine when connection is complete
    pub fn increment_byte_sent(&mut self) {
        self.bytes_sent += 1
    }
}

#[derive(Debug, Clone, Getters, MutGetters)]
pub struct ConnectionMetadata {
    #[getset(get = "pub", get_mut = "pub")]
    request_metadata: RequestMetadata,
    #[getset(get = "pub")]
    client_metadata: ClientMetadata,
}

impl ConnectionMetadata {
    pub fn new(request_metadata: RequestMetadata, client_metadata: ClientMetadata) -> Self {
        Self {
            request_metadata,
            client_metadata,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TarpitRequest {
    channel: Arc<Mutex<TarpitRecv>>,
    content_type: ContentType,
    payload_size: u64,
}

impl TarpitRequest {
    pub fn new(channel: TarpitRecv, content_type: ContentType, payload_size: u64) -> Self {
        Self {
            channel: Arc::new(Mutex::new(channel)),
            content_type,
            payload_size,
        }
    }

    pub fn content_type(&self) -> ContentType {
        self.content_type.to_owned()
    }

    pub fn payload_size(&self) -> u64 {
        self.payload_size
    }

    pub fn channel(&self) -> Arc<Mutex<TarpitRecv>> {
        self.channel.clone()
    }
}

#[derive(Debug, Clone, Getters)]
pub struct TarpitConnection {
    #[getset(get = "pub")]
    metadata: ConnectionMetadata,
    #[getset(get = "pub")]
    channel: TarpitSender,
}

impl TarpitConnection {
    pub fn new(response_size: u64, client_metadata: ClientMetadata) -> (Self, TarpitRecv) {
        let (channel, recv) = mpsc::unbounded_channel();
        let request_metadata = RequestMetadata::new(response_size);
        let metadata = ConnectionMetadata::new(request_metadata, client_metadata);
        (Self { metadata, channel }, recv)
    }

    pub fn get_conn_metadata(&self) -> &ClientMetadata {
        self.metadata.client_metadata()
    }

    pub fn should_send_byte(&mut self, duration_per_byte: Duration) -> bool {
        if let Some(time_since) = self.metadata.request_metadata().time_since_last_byte() {
            time_since.elapsed() >= duration_per_byte
        } else {
            true
        }
    }

    fn set_time_since_last_byte(&mut self, time: Instant) {
        self.metadata
            .request_metadata_mut()
            .set_time_since_last_byte(Some(time));
    }

    pub fn send_byte(&mut self, payload: Bytes) -> Result<(), Error> {
        self.channel.send(payload)?;
        self.set_time_since_last_byte(Instant::now());
        self.sent_byte();
        Ok(())
    }

    pub fn sent_byte(&mut self) {
        self.metadata.request_metadata_mut().increment_byte_sent();
    }

    fn bytes_sent(&self) -> u64 {
        self.metadata.request_metadata().bytes_sent()
    }

    fn response_size(&self) -> u64 {
        self.metadata.request_metadata().response_size()
    }

    pub fn should_abort(&self) -> bool {
        let payload_complete = self.bytes_sent() >= self.response_size();
        let chann_closed = self.channel.is_closed();
        chann_closed || payload_complete
    }
}
