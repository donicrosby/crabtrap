use super::{HostMetadata, RequestInfoMetadata, UserAgentMetadata, NO_METADATA_FOUND};
use crate::Req;
use std::task::{Context, Poll};
use tower::{Layer, Service};

#[derive(Debug, Clone)]
pub struct ClientMetadata {
    host: Option<HostMetadata>,
    user_agent: Option<UserAgentMetadata>,
    request_info: RequestInfoMetadata,
}

impl ClientMetadata {
    pub fn new(
        host: Option<HostMetadata>,
        user_agent: Option<UserAgentMetadata>,
        request_info: RequestInfoMetadata,
    ) -> Self {
        Self {
            host,
            user_agent,
            request_info,
        }
    }

    pub fn host(&self) -> String {
        self.host
            .clone()
            .map(|metadata| metadata.host())
            .unwrap_or(Self::unknown_metadata())
    }

    pub fn user_agent_string(&self) -> String {
        self.user_agent
            .clone()
            .map(|metadata| metadata.user_agent())
            .unwrap_or(Self::unknown_metadata())
    }

    pub fn location(&self) -> String {
        self.request_info.location()
    }

    pub fn query(&self) -> String {
        self.request_info
            .query()
            .unwrap_or(Self::unknown_metadata())
    }

    pub fn method(&self) -> String {
        self.request_info.method()
    }

    #[inline]
    fn unknown_metadata() -> String {
        String::from(NO_METADATA_FOUND)
    }
}

#[derive(Debug, Clone)]
pub struct TarpitMetadataCollector<S> {
    inner: S,
}

impl<S> TarpitMetadataCollector<S> {
    pub fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<S> Service<Req> for TarpitMetadataCollector<S>
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
        let host = req.extensions_mut().remove::<HostMetadata>();
        let user_agent = req.extensions_mut().remove::<UserAgentMetadata>();
        let request_info = req
            .extensions_mut()
            .remove::<RequestInfoMetadata>()
            .expect("request info was not parsed");
        let metadata = ClientMetadata::new(host, user_agent, request_info);
        req.extensions_mut().insert(metadata);
        self.inner.call(req)
    }
}

#[derive(Debug, Clone)]
pub struct TarpitMetadataCollectorLayer;

impl<S> Layer<S> for TarpitMetadataCollectorLayer {
    type Service = TarpitMetadataCollector<S>;

    fn layer(&self, inner: S) -> Self::Service {
        TarpitMetadataCollector::new(inner)
    }
}
