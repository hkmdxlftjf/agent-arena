//! Shared SSE framing. Comment lines every 15s keep Cloudflare Tunnel from
//! treating the response as idle. Buffering headers tell proxies not to hold
//! the body.

use axum::body::Body;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::Response;

pub const KEEPALIVE_SECS: u64 = 15;

pub fn comment(text: &str) -> String {
    format!(": {text}\n\n")
}

pub fn event(name: &str, id: &str, data: &str) -> String {
    let mut out = String::new();
    if !id.is_empty() {
        out.push_str("id: ");
        out.push_str(id);
        out.push('\n');
    }
    out.push_str("event: ");
    out.push_str(name);
    out.push('\n');
    for line in data.split('\n') {
        out.push_str("data: ");
        out.push_str(line);
        out.push('\n');
    }
    out.push('\n');
    out
}

pub fn accepts_sse(headers: &HeaderMap) -> bool {
    headers
        .get("accept")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.contains("text/event-stream"))
        .unwrap_or(false)
}

pub fn response(
    stream: impl futures::Stream<Item = Result<String, std::io::Error>> + Send + 'static,
) -> Response {
    let body = Body::from_stream(stream);
    let mut res = Response::builder()
        .status(StatusCode::OK)
        .body(body)
        .unwrap();
    let headers = res.headers_mut();
    headers.insert(
        "content-type",
        HeaderValue::from_static("text/event-stream; charset=utf-8"),
    );
    headers.insert(
        "cache-control",
        HeaderValue::from_static("no-cache, no-store, no-transform"),
    );
    headers.insert("x-accel-buffering", HeaderValue::from_static("no"));
    headers.insert("cdn-cache-control", HeaderValue::from_static("no-store"));
    headers.insert(
        "cloudflare-cdn-cache-control",
        HeaderValue::from_static("no-store"),
    );
    headers.insert("connection", HeaderValue::from_static("keep-alive"));
    headers.insert("content-encoding", HeaderValue::from_static("identity"));
    res
}
