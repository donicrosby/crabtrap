use crate::TarpitConnSend;
use futures::stream::{self, Stream, TryStreamExt};
use http_body_util::StreamBody;
use hyper::body::{Bytes, Frame, Incoming};
use hyper::header::{self, HeaderMap, HeaderName};
use hyper::service::Service;
use hyper::{Request, Response, Uri};
use mime::{self, Mime};
use rand::prelude::*;
use std::convert::Infallible;
use std::str::FromStr;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::{mpsc, Mutex as TokioMutex};
use tokio::time::{Duration, Instant};

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

pub(crate) async fn tarpit_impl(
    req: Request<Incoming>,
) -> Result<Response<StreamBody<impl Stream<Item = Result<Frame<Bytes>, Infallible>>>>, Error> {
    let payload = req.extensions().get::<TarpitPayload>().unwrap().to_owned();
    let resp = Response::builder()
        .header(header::CONTENT_TYPE, payload.content_type.to_string())
        .header(header::CONTENT_LENGTH, payload.payload_size);
    let body_stream = stream::unfold(payload, move |payload| async move {
        payload
            .channel
            .lock()
            .await
            .recv()
            .await
            .map(|byte| (Ok(byte), payload.clone()))
    });
    let body = StreamBody::new(body_stream.map_ok(Frame::data));
    let resp = resp.body(body)?;
    Ok(resp)
}

pub(crate) type TarpitSender = mpsc::UnboundedSender<Bytes>;
pub(crate) type TarpitRecv = mpsc::UnboundedReceiver<Bytes>;

#[derive(Debug, Clone)]
pub(crate) struct TarpitConnection {
    bytes_sent: u64,
    response_size: u64,
    metadata: TarPitMetadata,
    time_since_last_byte: Option<Instant>,
    channel: TarpitSender,
}

impl TarpitConnection {
    pub fn new(response_size: u64, metadata: TarPitMetadata, channel: TarpitSender) -> Self {
        Self {
            bytes_sent: 0,
            time_since_last_byte: None,
            metadata,
            response_size,
            channel,
        }
    }

    pub fn get_conn_metadata(&self) -> &TarPitMetadata {
        &self.metadata
    }

    pub fn should_send_byte(&mut self, duration_per_byte: Duration) -> bool {
        if let Some(time_since) = self.time_since_last_byte {
            time_since.elapsed() >= duration_per_byte
        } else {
            true
        }
    }

    pub fn send_byte(&mut self, payload: Bytes) -> Result<(), Error> {
        self.channel.send(payload)?;
        self.time_since_last_byte = Some(Instant::now());
        self.sent_byte();
        Ok(())
    }

    pub fn sent_byte(&mut self) {
        self.bytes_sent += 1;
    }

    pub fn should_abort(&self) -> bool {
        let payload_complete = self.bytes_sent >= self.response_size;
        let chann_closed = self.channel.is_closed();
        chann_closed || payload_complete
    }
}

#[derive(Debug, Clone)]
pub struct ContentType {
    inner: Mime,
}

impl ContentType {
    pub fn new(str: &str) -> Result<Self, Error> {
        let inner = str.parse()?;
        Ok(Self { inner })
    }
}

impl From<Mime> for ContentType {
    fn from(value: Mime) -> Self {
        Self { inner: value }
    }
}

impl ToString for ContentType {
    fn to_string(&self) -> String {
        self.inner.to_string()
    }
}

impl FromStr for ContentType {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TarpitPayload {
    channel: Arc<TokioMutex<TarpitRecv>>,
    content_type: ContentType,
    payload_size: u64,
}

impl TarpitPayload {
    pub fn new(channel: TarpitRecv, content_type: ContentType, payload_size: u64) -> Self {
        Self {
            channel: Arc::new(TokioMutex::new(channel)),
            content_type,
            payload_size,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TarPitMetadata {
    host: Option<String>,
    user_agent_string: Option<String>,
    location: Option<String>,
    query: Option<String>,
}

impl TarPitMetadata {
    pub fn new(
        host: Option<String>,
        user_agent_string: Option<String>,
        location: Option<String>,
        query: Option<String>,
    ) -> Self {
        Self {
            host,
            user_agent_string,
            location,
            query,
        }
    }

    pub fn host(&self) -> String {
        self.host.clone().unwrap_or(String::from("N/A"))
    }

    pub fn user_agent_string(&self) -> String {
        self.user_agent_string
            .clone()
            .unwrap_or(String::from("N/A"))
    }

    pub fn location(&self) -> String {
        self.location.clone().unwrap_or(String::from("None"))
    }

    pub fn query(&self) -> String {
        self.query.clone().unwrap_or(String::from("None"))
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TarPitMetadataCollector<S> {
    inner: S,
}

impl<S> TarPitMetadataCollector<S> {
    pub fn new(inner: S) -> Self {
        Self { inner }
    }

    fn get_header(headers: &HeaderMap, header: HeaderName) -> Option<String> {
        headers
            .get(header)
            .and_then(|h| h.to_str().ok())
            .map(String::from)
    }
}

impl<S> Service<Req> for TarPitMetadataCollector<S>
where
    S: Service<Req>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn call(&self, req: Req) -> Self::Future {
        let uri_host = req.uri().host().map(String::from);
        let host_header = req
            .headers()
            .get(header::HOST)
            .map(|header| header.to_str().ok())
            .and_then(|host_header| {
                if let Some(host_header) = host_header {
                    Uri::builder()
                        .authority(host_header)
                        .build()
                        .map_or(None, |uri| uri.host().map(String::from))
                } else {
                    None
                }
            });

        let host = if uri_host.is_some() {
            uri_host
        } else {
            host_header
        };
        let user_agent_string = Self::get_header(req.headers(), header::USER_AGENT);
        let location = Some(String::from(req.uri().path()));
        let query = req.uri().query().map(String::from);
        let metadata = TarPitMetadata::new(host, user_agent_string, location, query);
        let mut new_req = req;
        new_req.extensions_mut().insert(metadata);
        self.inner.call(new_req)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TarPit<S> {
    send_conn: TarpitConnSend,
    content_type: ContentType,
    min_response_size: u64,
    max_response_size: u64,
    inner: S,
}

impl<S> TarPit<S> {
    pub fn new(
        min_response_size: u64,
        max_response_size: u64,
        content_type: ContentType,
        send_conn: TarpitConnSend,
        inner: S,
    ) -> Self {
        Self {
            min_response_size,
            max_response_size,
            content_type,
            send_conn,
            inner,
        }
    }
}

type Req = Request<Incoming>;

impl<S> Service<Req> for TarPit<S>
where
    S: Service<Req>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;
    fn call(&self, req: Req) -> Self::Future {
        let mut rng = thread_rng();
        let payload_size = rng.gen_range(self.min_response_size..=self.max_response_size);
        let (send, recv) = mpsc::unbounded_channel();
        let payload = TarpitPayload::new(recv, self.content_type.clone(), payload_size);
        let metadata = req.extensions().get::<TarPitMetadata>().unwrap();
        let connection = TarpitConnection::new(payload_size, metadata.clone(), send);
        self.send_conn
            .send(connection.clone())
            .expect("could not send connection to writer");
        let mut new_req = req;
        new_req.extensions_mut().insert(payload);
        self.inner.call(new_req)
    }
}
