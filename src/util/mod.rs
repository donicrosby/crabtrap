use hyper::body::Bytes;
use hyper::header::{HeaderMap, HeaderName};
use tokio::sync::mpsc;

mod content_type;

pub use self::content_type::ContentType;

pub type TarpitSender = mpsc::UnboundedSender<Bytes>;
pub type TarpitRecv = mpsc::UnboundedReceiver<Bytes>;

pub(crate) fn extract_header(headers: &HeaderMap, header: HeaderName) -> Option<String> {
    headers
        .get(header)
        .and_then(|h| h.to_str().ok())
        .map(String::from)
}
