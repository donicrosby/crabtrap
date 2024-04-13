use crate::Req;
use hyper::Method;
use std::task::{Context, Poll};
use tower::{Layer, Service};

#[derive(Debug, Clone)]
pub struct RequestInfoMetadata {
    method: Method,
    location: String,
    query: Option<String>,
}

impl RequestInfoMetadata {
    pub fn new(method: Method, location: String, query: Option<String>) -> Self {
        Self {
            method,
            location,
            query,
        }
    }

    pub fn method(&self) -> String {
        self.method.to_string()
    }

    pub fn location(&self) -> String {
        self.location.clone()
    }

    pub fn query(&self) -> Option<String> {
        self.query.clone()
    }
}

#[derive(Debug, Clone)]
pub struct RequestInfoExtractor<S> {
    inner: S,
}

impl<S> RequestInfoExtractor<S> {
    pub fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<S> Service<Req> for RequestInfoExtractor<S>
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
        let method = req.method().to_owned();
        let location = String::from(req.uri().path());
        let query = req.uri().query().map(String::from);
        let request_info_metadata = RequestInfoMetadata::new(method, location, query);
        let mut req = req;
        req.extensions_mut().insert(request_info_metadata);
        self.inner.call(req)
    }
}

#[derive(Debug, Clone)]
pub struct RequestInfoExtractorLayer;

impl<S> Layer<S> for RequestInfoExtractorLayer {
    type Service = RequestInfoExtractor<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RequestInfoExtractor::new(inner)
    }
}
