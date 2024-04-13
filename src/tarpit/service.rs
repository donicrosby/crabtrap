use crate::{ClientMetadata, Error, TarpitConfig, TarpitConnection, TarpitRequest};
use futures::stream::{self, TryStreamExt};
use http_body_util::StreamBody;
use hyper::body::Body;
use hyper::body::{Bytes, Frame, Incoming};
use hyper::header;
use hyper::{Request, Response};
use rand::distributions::Alphanumeric;
use rand::prelude::*;
use std::collections::VecDeque;
use std::convert::Infallible;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::runtime::Handle;
use tokio::task::{block_in_place, JoinHandle};
use tokio::{sync::Mutex, time};
use tokio_stream::{self as tokio_stream, Stream, StreamExt};
use tower::Service;
use tracing::{debug, info, trace, warn};

pub async fn handle_tarpit_connection(
    req: Request<Incoming>,
) -> Result<Response<StreamBody<impl Stream<Item = Result<Frame<Bytes>, Infallible>>>>, Error> {
    let mut req = req;
    let payload = req
        .extensions_mut()
        .remove::<TarpitRequest>()
        .expect("no payload given");
    let resp = Response::builder()
        .header(header::CONTENT_TYPE, payload.content_type().to_string())
        .header(header::CONTENT_LENGTH, payload.payload_size());
    let body_stream = stream::unfold(payload, move |payload| async move {
        payload
            .channel()
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

#[atomic_enum::atomic_enum]
#[derive(PartialEq)]
enum TarpitStatus {
    Stopped = 0,
    Running,
}

impl Default for TarpitStatus {
    fn default() -> Self {
        Self::Stopped
    }
}

impl Default for AtomicTarpitStatus {
    fn default() -> Self {
        Self::new(TarpitStatus::default())
    }
}

#[derive(Debug, Default)]
struct WriterState {
    connections: Mutex<VecDeque<TarpitConnection>>,
    status: AtomicTarpitStatus,
    job: Mutex<Option<JoinHandle<()>>>,
}

#[derive(Debug, Clone)]
pub struct Tarpit<S> {
    state: Arc<WriterState>,
    config: TarpitConfig,
    inner: S,
}

impl<S> Tarpit<S> {
    pub fn new(inner: S, config: TarpitConfig) -> Self {
        Self {
            state: Arc::new(WriterState::default()),
            config,
            inner,
        }
    }

    fn get_num_time_slices(&self) -> u128 {
        self.config.duration_per_byte().as_millis() / self.config.tick_duration().as_millis()
    }

    fn num_connections_per_time_slice(&self, num_slices: u128, num_conns: usize) -> usize {
        let div = num_conns / num_slices as usize;
        if div > 0 {
            div
        } else if num_conns > 0 {
            num_conns
        } else {
            0
        }
    }

    pub async fn add_new_conn(&mut self, conn: TarpitConnection) {
        debug!("Received new connection...");
        Self::display_connection_info(conn.get_conn_metadata());
        self.state.connections.lock().await.push_back(conn);
    }

    pub async fn add_handle(&mut self, handle: JoinHandle<()>) {
        let _handle = self.state.job.lock().await.insert(handle);
    }

    pub async fn _shutdown(&mut self) -> JoinHandle<()> {
        let handle = self
            .state
            .job
            .lock()
            .await
            .take()
            .expect("tarpit was already stopped");
        self.state
            .status
            .store(TarpitStatus::Stopped, Ordering::Release);
        handle
    }

    pub async fn process_connections(&self) {
        self.state
            .status
            .store(TarpitStatus::Running, Ordering::Release);
        let mut time_slice = time::interval(self.config.tick_duration());
        loop {
            time_slice.tick().await;
            trace!("Writer waking up...");
            {
                let mut conns = self.state.connections.lock().await;
                let num_to_process =
                    self.num_connections_per_time_slice(self.get_num_time_slices(), conns.len());
                let to_process = conns.drain(..num_to_process);
                let work_stream = stream::iter(to_process);
                let mut processed_connections = work_stream
                    .filter_map(|mut conn| {
                        if conn.should_send_byte(self.config.duration_per_byte()) {
                            let char: Bytes = rand::thread_rng()
                                .clone()
                                .sample_iter(&Alphanumeric)
                                .take(1)
                                .collect();
                            if conn.send_byte(char).is_ok() {
                                trace!("Byte sent successfully");
                            } else {
                                warn!("Byte failed to send");
                            }
                        }
                        if !conn.should_abort() {
                            Some(conn)
                        } else {
                            info!("Connection complete, cleaning up...");
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .await
                    .into();
                conns.append(&mut processed_connections);
            }
        }
    }

    fn display_connection_info(metadata: &ClientMetadata) {
        let host = metadata.host();
        let user_agent = metadata.user_agent_string();
        let path = metadata.location();
        let query = metadata.query();
        let method = metadata.method();
        info!(
            "Connection: {{ Host: {}, User Agent: {}, Method: {}, Path: {}, Query: {} }}",
            host, user_agent, method, path, query
        );
    }
}

impl<S, B> Service<Request<B>> for Tarpit<S>
where
    S: Service<Request<B>> + Clone,
    B: Body,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        match self.state.status.load(Ordering::Acquire) {
            TarpitStatus::Running => self.inner.poll_ready(cx),
            TarpitStatus::Stopped => Poll::Pending,
        }
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
        let mut rng = rand::thread_rng();
        let response_size =
            rng.gen_range(self.config.min_body_size()..=self.config.max_body_size());
        let mut req = req;
        let client_metadata: ClientMetadata = req
            .extensions_mut()
            .remove::<ClientMetadata>()
            .expect("no metadata given");
        let (self_ret, req) = block_in_place(move || {
            Handle::current().block_on(async move {
                let (conn, send) = TarpitConnection::new(response_size, client_metadata);
                self.add_new_conn(conn).await;
                let payload = TarpitRequest::new(send, self.config.content_type(), response_size);
                let mut req = req;
                req.extensions_mut().insert(payload);
                (self, req)
            })
        });
        self_ret.inner.call(req)
    }
}
