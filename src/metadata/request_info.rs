use hyper::body::Body;
use hyper::Method;
use hyper::Request;
use std::task::{Context, Poll};
use tower::{Layer, Service};

#[derive(Debug, Clone, PartialEq)]
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

impl<S, B> Service<Request<B>> for RequestInfoExtractor<S>
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
    async fn test_extract_request_info() {
        let request = Request::builder()
            .method(Method::GET)
            .uri("http://localhost/test?query=test_query")
            .body(Empty::<Bytes>::default())
            .unwrap();
        let expected = RequestInfoMetadata::new(
            Method::GET,
            String::from("/test"),
            Some(String::from("query=test_query")),
        );

        let (svc, mut req_handle) = mock::pair::<Request<Empty<Bytes>>, Response<Empty<Bytes>>>();
        let svc = ServiceBuilder::new()
            .layer(RequestInfoExtractorLayer)
            .service(svc);
        let mut svc = Spawn::new(svc);

        assert_ready!(svc.poll_ready()).expect("extractor not ready");

        let res_fut = svc.call(request);
        let (req, res_handle) = req_handle.next_request().await.unwrap();
        let parsed = req
            .extensions()
            .get::<RequestInfoMetadata>()
            .expect("extractor did not parse the request info")
            .clone();

        assert_eq!(expected, parsed);
        res_handle.send_response(Response::builder().body(Empty::<Bytes>::default()).unwrap());
        res_fut.await.expect("failure to run future");
    }

    #[tokio::test]
    async fn test_extract_request_info_no_query() {
        let request = Request::builder()
            .method(Method::POST)
            .uri("http://localhost/test")
            .body(Empty::<Bytes>::default())
            .unwrap();
        let expected = RequestInfoMetadata::new(Method::POST, String::from("/test"), None);

        let (svc, mut req_handle) = mock::pair::<Request<Empty<Bytes>>, Response<Empty<Bytes>>>();
        let svc = ServiceBuilder::new()
            .layer(RequestInfoExtractorLayer)
            .service(svc);
        let mut svc = Spawn::new(svc);

        assert_ready!(svc.poll_ready()).expect("extractor not ready");

        let res_fut = svc.call(request);
        let (req, res_handle) = req_handle.next_request().await.unwrap();
        let parsed = req
            .extensions()
            .get::<RequestInfoMetadata>()
            .expect("extractor did not parse the request info")
            .clone();

        assert_eq!(expected, parsed);
        res_handle.send_response(Response::builder().body(Empty::<Bytes>::default()).unwrap());
        res_fut.await.expect("failure to run future");
    }
}
