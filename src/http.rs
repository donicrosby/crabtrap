use std::convert::Infallible;

use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
use hyper::{Request, Response};

pub(crate) async fn hello(_: Request<Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    Ok(Response::new(Full::new(Bytes::from("Hello, World!"))))
}
