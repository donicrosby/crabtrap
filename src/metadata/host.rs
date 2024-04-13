use hyper::body::Body;
use hyper::header;
use hyper::Request;
use hyper::Uri;
use std::task::{Context, Poll};
use tower::{Layer, Service};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostMetadata {
    host: String,
}

impl HostMetadata {
    pub fn new(host: String) -> Self {
        Self { host }
    }

    pub fn host(&self) -> String {
        self.host.clone()
    }
}

#[derive(Debug, Clone)]
pub struct HostExtractor<S> {
    inner: S,
}

impl<S> HostExtractor<S> {
    pub fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<S, B> Service<Request<B>> for HostExtractor<S>
where
    S: Service<Request<B>> + Clone,
    B: Body,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
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

        let mut req = req;
        if let Some(host) = host {
            let host_metadata = HostMetadata::new(host);
            req.extensions_mut().insert(host_metadata);
        }
        self.inner.call(req)
    }
}

#[derive(Debug, Clone)]
pub struct HostExtractorLayer;

impl<S> Layer<S> for HostExtractorLayer {
    type Service = HostExtractor<S>;

    fn layer(&self, inner: S) -> Self::Service {
        HostExtractor::new(inner)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use bytes::Bytes;
    use http_body_util::Empty;
    use hyper::{Request, Response};
    use tokio_test::assert_ready;
    use tower::ServiceBuilder;
    use tower_test::mock::{self, Spawn};

    #[tokio::test]
    async fn test_extract_from_uri() {
        let request = Request::builder()
            .uri("http://localhost")
            .body(Empty::<Bytes>::default())
            .unwrap();
        let expected = HostMetadata::new(String::from("localhost"));

        let (svc, mut req_handle) = mock::pair::<Request<Empty<Bytes>>, Response<Empty<Bytes>>>();
        let svc = ServiceBuilder::new().layer(HostExtractorLayer).service(svc);
        let mut svc = Spawn::new(svc);

        assert_ready!(svc.poll_ready()).expect("extractor not ready");

        let res_fut = svc.call(request);
        let (req, res_handle) = req_handle.next_request().await.unwrap();
        let parsed = req
            .extensions()
            .get::<HostMetadata>()
            .expect("extractor did not parse the hostname")
            .clone();

        assert_eq!(expected, parsed);
        res_handle.send_response(Response::builder().body(Empty::<Bytes>::default()).unwrap());
        res_fut.await.expect("failure to run future");
    }

    #[tokio::test]
    async fn test_extract_from_header() {
        let request = Request::builder()
            .header(header::HOST, String::from("localhost"))
            .body(Empty::<Bytes>::default())
            .unwrap();
        let expected = HostMetadata::new(String::from("localhost"));

        let (svc, mut req_handle) = mock::pair::<Request<Empty<Bytes>>, Response<Empty<Bytes>>>();
        let svc = ServiceBuilder::new().layer(HostExtractorLayer).service(svc);
        let mut svc = Spawn::new(svc);

        assert_ready!(svc.poll_ready()).expect("extractor not ready");

        let res_fut = svc.call(request);
        let (req, res_handle) = req_handle.next_request().await.unwrap();
        let parsed = req
            .extensions()
            .get::<HostMetadata>()
            .expect("extractor did not parse the hostname")
            .clone();

        assert_eq!(expected, parsed);
        res_handle.send_response(Response::builder().body(Empty::<Bytes>::default()).unwrap());
        res_fut.await.expect("failure to run future");
    }
}
