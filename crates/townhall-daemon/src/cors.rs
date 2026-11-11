use axum::http::{header, HeaderMap, HeaderName, HeaderValue, Method};
use tower_http::cors::{AllowOrigin, CorsLayer};

use crate::middleware::REQUEST_ID_HEADER;

/// Origins allowed to call the API from a browser context: the desktop shell
/// (packaged and dev) and the Vite dev server. Nothing else.
pub const ALLOWED_ORIGINS: [&str; 5] = [
    "tauri://localhost",
    "http://tauri.localhost",
    "https://tauri.localhost",
    "http://localhost:5173",
    "http://127.0.0.1:5173",
];

pub fn cors_layer() -> CorsLayer {
    let origins: Vec<HeaderValue> = ALLOWED_ORIGINS
        .iter()
        .map(|o| HeaderValue::from_static(o))
        .collect();
    CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
        ])
        .allow_headers([
            header::CONTENT_TYPE,
            header::AUTHORIZATION,
            HeaderName::from_static(REQUEST_ID_HEADER),
        ])
}

/// False when a browser page from another origin sent the request. CORS does not cover
/// websockets or form-like posts (multipart), so `/api/events` and uploads check this.
/// No `Origin` header (curl, claude's MCP client) is allowed.
pub fn origin_allowed(headers: &HeaderMap) -> bool {
    match headers.get(header::ORIGIN) {
        None => true,
        Some(origin) => ALLOWED_ORIGINS.iter().any(|o| origin == o),
    }
}

pub fn browser_origin(headers: &HeaderMap) -> bool {
    headers.contains_key(header::ORIGIN) && origin_allowed(headers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_check() {
        let with = |o: &'static str| {
            let mut h = HeaderMap::new();
            h.insert(header::ORIGIN, HeaderValue::from_static(o));
            h
        };
        assert!(origin_allowed(&HeaderMap::new()));
        assert!(origin_allowed(&with("tauri://localhost")));
        assert!(origin_allowed(&with("http://localhost:5173")));
        assert!(!origin_allowed(&with("https://evil.example")));
    }
}
