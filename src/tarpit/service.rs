use crate::{ClientMetadata, Error, TarpitConfig};
use futures::{stream, Stream, StreamExt};
use http_body_util::StreamBody;
use hyper::{
    body::{Body, Bytes, Frame},
    header, Request, Response,
};
use rand::{distributions::Alphanumeric, prelude::*};
use std::pin::Pin;
use std::{
    convert::Infallible,
    task::{Context, Poll},
};
use tokio::time::{self, Interval};
use tower::Service;

use tracing::{debug, info, trace, warn};

#[derive(Debug)]
struct TarpitStream {
    interval: time::Interval,
    bytes_to_send: u64,
    bytes_sent: u64,
}

impl TarpitStream {
    pub fn new(interval: Interval, bytes_to_send: u64) -> Self {
        Self {
            interval,
            bytes_to_send,
            bytes_sent: 0,
        }
    }

    #[inline(always)]
    pub fn sent_byte(&mut self) {
        self.bytes_sent += 1;
    }

    #[inline(always)]
    pub fn finished(&self) -> bool {
        self.bytes_sent >= self.bytes_to_send
    }

    pub async fn tick(&mut self) {
        self.interval.tick().await;
    }
}

#[derive(Debug, Clone)]
pub struct Tarpit {
    config: TarpitConfig,
}

impl Tarpit {
    pub fn new(config: TarpitConfig) -> Self {
        Self { config }
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

impl<B> Service<Request<B>> for Tarpit
where
    B: Body,
{
    type Response =
        Response<StreamBody<Pin<Box<dyn Stream<Item = Result<Frame<Bytes>, Infallible>> + Send>>>>;
    type Error = Error;
    type Future = Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send + Sync>,
    >;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
        let content_type = self.config.content_type();
        let mut rng = rand::thread_rng();
        let response_size =
            rng.gen_range(self.config.min_body_size()..=self.config.max_body_size());
        let duration_per_byte = self.config.duration_per_byte();

        let mut req = req;
        let client_metadata: ClientMetadata = req
            .extensions_mut()
            .remove::<ClientMetadata>()
            .expect("no metadata given");

        let fut = async move {
            Self::display_connection_info(&client_metadata);

            let resp = Response::builder()
                .header(header::CONTENT_TYPE, content_type.to_string())
                .header(header::CONTENT_LENGTH, response_size);
            let tarpit_stream = TarpitStream::new(time::interval(duration_per_byte), response_size);
            let body_stream = stream::unfold(
                tarpit_stream,
                move |mut tarpit_stream: TarpitStream| async move {
                    if tarpit_stream.finished() {
                        None
                    } else {
                        tarpit_stream.tick().await;
                        let char: Bytes = rand::thread_rng()
                            .clone()
                            .sample_iter(&Alphanumeric)
                            .take(1)
                            .collect();
                        let frame = Frame::data(char);
                        tarpit_stream.sent_byte();
                        Some((Ok(frame), tarpit_stream))
                    }
                },
            )
            .boxed();
            let body = StreamBody::new(body_stream);
            let resp = resp.body(body)?;
            Ok(resp)
        };

        Box::pin(fut)
    }
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use super::*;
    use crate::{ContentType, HostMetadata, RequestInfoMetadata, TarpitConfig, UserAgentMetadata};
    use bytes::Bytes;
    use http_body_util::Empty;
    use hyper::{Method, Request};
    use mime;
    use tokio_stream::StreamExt;
    use tower_test::mock::Spawn;

    #[tokio::test]
    async fn test_process_tarpit() {
        let metadata = ClientMetadata::new(Some(HostMetadata::from_str("localhost").unwrap()), Some(UserAgentMetadata::from_str("Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:124.0) Gecko/20100101 Firefox/124.0").unwrap()), RequestInfoMetadata::new(Method::GET, String::from("/"), None));
        let request = Request::builder()
            .extension(metadata)
            .body(Empty::<Bytes>::default())
            .unwrap();
        let config = TarpitConfig::new(
            10,
            10,
            1,
            1,
            ContentType::new(&mime::TEXT_PLAIN.to_string()).unwrap(),
        );

        let tarpit = Tarpit::new(config);

        let mut svc = Spawn::new(tarpit);

        let res = svc.call(request).await;
        assert!(res.is_ok());
        let res = res.unwrap();
        let (_, body) = res.into_parts();
        let bytes: Vec<u8> = body
            .fold(Vec::new(), |mut vec, byte| {
                let raw_byte = byte.unwrap().into_data().unwrap();
                let mut byte_vec = raw_byte.iter().cloned().collect::<Vec<u8>>();
                vec.append(&mut byte_vec);
                vec
            })
            .await;
        assert_eq!(bytes.len(), 10);
    }
}
