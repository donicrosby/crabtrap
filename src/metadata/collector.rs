use super::{HostMetadata, RequestInfoMetadata, UserAgentMetadata, NO_METADATA_FOUND};
use hyper::body::Body;
use hyper::Request;
use std::task::{Context, Poll};
use tower::{Layer, Service};

#[derive(Debug, Clone, PartialEq)]
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

impl<S, B> Service<Request<B>> for TarpitMetadataCollector<S>
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

#[cfg(test)]
mod test {
    use super::*;
    use bytes::Bytes;
    use http_body_util::Empty;
    use hyper::{Method, Request, Response};
    use tokio_test::assert_ready;
    use tower::ServiceBuilder;
    use tower_test::mock::{self, Spawn};

    #[tokio::test]
    async fn test_collect_metadata() {
        let host = HostMetadata::new(String::from("localhost"));
        let user_agent = UserAgentMetadata::new(String::from(
            "Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:124.0) Gecko/20100101 Firefox/124.0",
        ));
        let req_info = RequestInfoMetadata::new(Method::GET, String::from("/"), None);
        let request = Request::builder()
            .extension(host.clone())
            .extension(user_agent.clone())
            .extension(req_info.clone())
            .body(Empty::<Bytes>::default())
            .unwrap();
        let expected = ClientMetadata::new(Some(host), Some(user_agent), req_info);

        let (svc, mut req_handle) = mock::pair::<Request<Empty<Bytes>>, Response<Empty<Bytes>>>();
        let svc = ServiceBuilder::new()
            .layer(TarpitMetadataCollectorLayer)
            .service(svc);
        let mut svc = Spawn::new(svc);

        assert_ready!(svc.poll_ready()).expect("extractor not ready");

        let res_fut = svc.call(request);
        let (req, res_handle) = req_handle.next_request().await.unwrap();
        let parsed = req
            .extensions()
            .get::<ClientMetadata>()
            .expect("extractor did not parse the user agent")
            .clone();

        assert_eq!(expected, parsed);
        res_handle.send_response(Response::builder().body(Empty::<Bytes>::default()).unwrap());
        res_fut.await.expect("failure to run future");
    }

    #[tokio::test]
    async fn test_collect_metadata_no_host() {
        let user_agent = UserAgentMetadata::new(String::from(
            "Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:124.0) Gecko/20100101 Firefox/124.0",
        ));
        let req_info = RequestInfoMetadata::new(Method::GET, String::from("/"), None);
        let request = Request::builder()
            .extension(user_agent.clone())
            .extension(req_info.clone())
            .body(Empty::<Bytes>::default())
            .unwrap();
        let expected = ClientMetadata::new(None, Some(user_agent), req_info);

        let (svc, mut req_handle) = mock::pair::<Request<Empty<Bytes>>, Response<Empty<Bytes>>>();
        let svc = ServiceBuilder::new()
            .layer(TarpitMetadataCollectorLayer)
            .service(svc);
        let mut svc = Spawn::new(svc);

        assert_ready!(svc.poll_ready()).expect("extractor not ready");

        let res_fut = svc.call(request);
        let (req, res_handle) = req_handle.next_request().await.unwrap();
        let parsed = req
            .extensions()
            .get::<ClientMetadata>()
            .expect("extractor did not parse the user agent")
            .clone();

        assert_eq!(expected, parsed);
        res_handle.send_response(Response::builder().body(Empty::<Bytes>::default()).unwrap());
        res_fut.await.expect("failure to run future");
    }

    #[tokio::test]
    async fn test_collect_metadata_no_user_agent() {
        let host = HostMetadata::new(String::from("localhost"));
        let req_info = RequestInfoMetadata::new(Method::GET, String::from("/"), None);
        let request = Request::builder()
            .extension(host.clone())
            .extension(req_info.clone())
            .body(Empty::<Bytes>::default())
            .unwrap();
        let expected = ClientMetadata::new(Some(host), None, req_info);

        let (svc, mut req_handle) = mock::pair::<Request<Empty<Bytes>>, Response<Empty<Bytes>>>();
        let svc = ServiceBuilder::new()
            .layer(TarpitMetadataCollectorLayer)
            .service(svc);
        let mut svc = Spawn::new(svc);

        assert_ready!(svc.poll_ready()).expect("extractor not ready");

        let res_fut = svc.call(request);
        let (req, res_handle) = req_handle.next_request().await.unwrap();
        let parsed = req
            .extensions()
            .get::<ClientMetadata>()
            .expect("extractor did not parse the user agent")
            .clone();

        assert_eq!(expected, parsed);
        res_handle.send_response(Response::builder().body(Empty::<Bytes>::default()).unwrap());
        res_fut.await.expect("failure to run future");
    }

    #[tokio::test]
    #[should_panic]
    async fn test_collect_metadata_no_request_info() {
        let host = HostMetadata::new(String::from("localhost"));
        let user_agent = UserAgentMetadata::new(String::from(
            "Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:124.0) Gecko/20100101 Firefox/124.0",
        ));
        let request = Request::builder()
            .extension(host.clone())
            .extension(user_agent)
            .body(Empty::<Bytes>::default())
            .unwrap();

        let (svc, mut req_handle) = mock::pair::<Request<Empty<Bytes>>, Response<Empty<Bytes>>>();
        let svc = ServiceBuilder::new()
            .layer(TarpitMetadataCollectorLayer)
            .service(svc);
        let mut svc = Spawn::new(svc);

        assert_ready!(svc.poll_ready()).expect("extractor not ready");

        let res_fut = svc.call(request);
        let (_req, res_handle) = req_handle.next_request().await.unwrap();

        res_handle.send_response(Response::builder().body(Empty::<Bytes>::default()).unwrap());
        res_fut.await.expect("failure to run future");
    }
}
