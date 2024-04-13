use crate::extract_header;
use hyper::body::Body;
use hyper::header;
use hyper::Request;
use std::task::{Context, Poll};
use tower::{Layer, Service};

#[derive(Debug, Clone, PartialEq)]
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

impl<B, S> Service<Request<B>> for UserAgentExtractor<S>
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
    async fn test_extract_user_agent() {
        let request = Request::builder()
            .header(header::USER_AGENT, String::from("Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:124.0) Gecko/20100101 Firefox/124.0"))
            .body(Empty::<Bytes>::default())
            .unwrap();
        
        let expected = UserAgentMetadata::new(String::from(
            "Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:124.0) Gecko/20100101 Firefox/124.0",
        ));

        let (svc, mut req_handle) = mock::pair::<Request<Empty<Bytes>>, Response<Empty<Bytes>>>();
        let svc = ServiceBuilder::new()
            .layer(UserAgentExtractorLayer)
            .service(svc);
        let mut svc = Spawn::new(svc);

        assert_ready!(svc.poll_ready()).expect("extractor not ready");

        let res_fut = svc.call(request);
        let (req, res_handle) = req_handle.next_request().await.unwrap();
        let parsed = req
            .extensions()
            .get::<UserAgentMetadata>()
            .expect("extractor did not parse the user agent")
            .clone();

        assert_eq!(expected, parsed);
        res_handle.send_response(Response::builder().body(Empty::<Bytes>::default()).unwrap());
        res_fut.await.expect("failure to run future");
    }
}
