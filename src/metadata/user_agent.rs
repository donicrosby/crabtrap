use crate::{extract_header, Req};
use hyper::header;
use std::task::{Context, Poll};
use tower::{Layer, Service};

#[derive(Debug, Clone)]
pub struct UserAgentMetadata {
    user_agent: String,
}

impl UserAgentMetadata {
    pub fn new(user_agent: String) -> Self {
        Self { user_agent }
    }

    pub fn user_agent(&self) -> String {
        self.user_agent.clone()
    }
}

#[derive(Debug, Clone)]
pub struct UserAgentExtractor<S> {
    inner: S,
}

impl<S> UserAgentExtractor<S> {
    pub fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<S> Service<Req> for UserAgentExtractor<S>
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
        let mut req = req;
        if let Some(user_agent) = extract_header(req.headers(), header::USER_AGENT) {
            let user_agent_metadata = UserAgentMetadata::new(user_agent);
            req.extensions_mut().insert(user_agent_metadata);
        }
        self.inner.call(req)
    }
}

#[derive(Debug, Clone)]
pub struct UserAgentExtractorLayer;

impl<S> Layer<S> for UserAgentExtractorLayer {
    type Service = UserAgentExtractor<S>;

    fn layer(&self, inner: S) -> Self::Service {
        UserAgentExtractor::new(inner)
    }
}
