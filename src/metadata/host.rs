use crate::Req;
use hyper::header;
use hyper::Uri;
use std::task::{Context, Poll};
use tower::{Layer, Service};

#[derive(Debug, Clone)]
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

impl<S> Service<Req> for HostExtractor<S>
where
    S: Service<Req> + Clone,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Req) -> Self::Future {
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
