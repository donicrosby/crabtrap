use hyper::header::{HeaderMap, HeaderName};

mod content_type;

pub use self::content_type::ContentType;

pub(crate) fn extract_header(headers: &HeaderMap, header: HeaderName) -> Option<String> {
    headers
        .get(header)
        .and_then(|h| h.to_str().ok())
        .map(String::from)
}
