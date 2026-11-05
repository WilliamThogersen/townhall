//! HTTP request logging and request ids. See `docs/logging.md` for the events.

use std::time::Instant;

use axum::extract::{MatchedPath, Request};
use axum::http::{HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use townhall_core::redact::redact;

pub const REQUEST_ID_HEADER: &str = "x-request-id";
const MAX_REQUEST_ID: usize = 64;

/// Logs one `http.request` per request and echoes `x-request-id` (the caller's, or a new one).
/// Websocket routes log `ws.open`/`ws.close` themselves, so a 101 is not logged here.
pub async fn log_requests(mut req: Request, next: Next) -> Response {
    let started = Instant::now();
    let method = req.method().clone();
    let path = req.uri().path().to_owned();
    let route = req
        .extensions()
        .get::<MatchedPath>()
        .map(|m| m.as_str().to_owned());
    let request_id = req
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|v| v.to_str().ok())
        .filter(|v| usable_id(v))
        .map(str::to_owned)
        .unwrap_or_else(townhall_core::ids::new_id);
    if let Ok(value) = HeaderValue::from_str(&request_id) {
        req.headers_mut().insert(REQUEST_ID_HEADER, value);
    }

    let mut res = next.run(req).await;
    let status = res.status();
    if let Ok(value) = HeaderValue::from_str(&request_id) {
        res.headers_mut().insert(REQUEST_ID_HEADER, value);
    }
    if status == StatusCode::SWITCHING_PROTOCOLS {
        return res;
    }

    let ids = path_ids(route.as_deref(), &path);
    let template = route.as_deref().unwrap_or("unmatched");
    let (conversation, agent, task) = (
        ids.conversation.as_deref(),
        ids.agent.as_deref(),
        ids.task.as_deref(),
    );
    let duration_ms = started.elapsed().as_millis() as u64;
    macro_rules! emit {
        ($level:ident) => {
            tracing::$level!(
                target: "townhall::http",
                event = "http.request",
                method = %method,
                route = template,
                status = status.as_u16(),
                duration_ms,
                request_id = %redact(&request_id),
                conversation,
                agent,
                task,
                "{} {} {}", method, template, status.as_u16()
            )
        };
    }
    if status.is_server_error() {
        emit!(error)
    } else if status.is_client_error() {
        emit!(info)
    } else {
        emit!(debug)
    }
    res
}

fn usable_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= MAX_REQUEST_ID && id.bytes().all(|b| b.is_ascii_graphic())
}

#[derive(Debug, Default, PartialEq)]
struct PathIds {
    conversation: Option<String>,
    agent: Option<String>,
    task: Option<String>,
}

/// The conversation, agent and task ids in `path`, read off the matched `route` template.
fn path_ids(route: Option<&str>, path: &str) -> PathIds {
    let mut ids = PathIds::default();
    let Some(route) = route else { return ids };
    let params = route.split('/').zip(path.split('/'));
    for (index, (template, actual)) in params.enumerate() {
        let Some(key) = template.strip_prefix('{').and_then(|t| t.strip_suffix('}')) else {
            continue;
        };
        let collection = route.split('/').nth(index.saturating_sub(1)).unwrap_or("");
        let value = Some(redact(actual).into_owned());
        match (collection, key) {
            ("conversations", "id") => ids.conversation = value,
            ("agents", "name") => ids.agent = value,
            ("tasks", "name") => ids.task = value,
            _ => {}
        }
    }
    ids
}

/// A websocket session: logs `ws.open` now and `ws.close` with the session length on drop.
pub struct WsSession {
    route: &'static str,
    agent: Option<String>,
    request_id: Option<String>,
    started: Instant,
}

impl WsSession {
    pub fn open(route: &'static str, headers: &axum::http::HeaderMap, agent: Option<&str>) -> Self {
        let session = Self {
            route,
            agent: agent.map(str::to_owned),
            request_id: headers
                .get(REQUEST_ID_HEADER)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned),
            started: Instant::now(),
        };
        tracing::info!(
            target: "townhall::http",
            event = "ws.open",
            route,
            agent = session.agent.as_deref(),
            request_id = session.request_id.as_deref(),
            "websocket open {route}"
        );
        session
    }
}

impl Drop for WsSession {
    fn drop(&mut self) {
        tracing::info!(
            target: "townhall::http",
            event = "ws.close",
            route = self.route,
            agent = self.agent.as_deref(),
            request_id = self.request_id.as_deref(),
            duration_ms = self.started.elapsed().as_millis() as u64,
            "websocket closed {}", self.route
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ApiError;
    use crate::logging::test_capture;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::routing::get;
    use axum::Router;
    use tower::ServiceExt;
    use townhall_core::Error;

    fn app() -> Router {
        Router::new()
            .route(
                "/api/tasks/{name}",
                get(|| async { Err::<(), _>(ApiError::Core(Error::missing("task", "x"))) }),
            )
            .route(
                "/api/boom",
                get(|| async {
                    Err::<(), _>(ApiError::Core(Error::Io {
                        context: "x".into(),
                        source: std::io::Error::other("disk"),
                    }))
                }),
            )
            .route("/api/ok", get(|| async { "fine" }))
            .layer(axum::middleware::from_fn(log_requests))
    }

    async fn get_uri(uri: &str, id: Option<&str>) -> axum::response::Response {
        let mut req = Request::builder().uri(uri);
        if let Some(id) = id {
            req = req.header(REQUEST_ID_HEADER, id);
        }
        app()
            .oneshot(req.body(Body::empty()).unwrap())
            .await
            .unwrap()
    }

    #[tokio::test(flavor = "current_thread")]
    async fn response_carries_a_generated_or_echoed_request_id() {
        let res = get_uri("/api/ok", None).await;
        assert!(!res.headers()[REQUEST_ID_HEADER].is_empty());
        let res = get_uri("/api/ok", Some("abc-123")).await;
        assert_eq!(res.headers()[REQUEST_ID_HEADER], "abc-123");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_404_logs_info_with_the_route_template() {
        let (capture, _guard) = test_capture();
        let res = get_uri("/api/tasks/nope", Some("rid-1")).await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
        let lines = capture.events("http.request");
        assert_eq!(lines.len(), 1, "{:?}", capture.lines());
        let l = &lines[0];
        assert_eq!(l["level"], "INFO");
        assert_eq!(l["target"], "townhall::http");
        assert_eq!(l["route"], "/api/tasks/{name}");
        assert_eq!(l["method"], "GET");
        assert_eq!(l["status"], 404);
        assert_eq!(l["task"], "nope");
        assert_eq!(l["request_id"], "rid-1");
        assert!(l["duration_ms"].is_number());
    }

    #[test]
    fn ws_session_logs_open_and_close() {
        let (capture, _guard) = test_capture();
        drop(WsSession::open(
            "/api/events",
            &axum::http::HeaderMap::new(),
            None,
        ));
        assert_eq!(capture.events("ws.open").len(), 1);
        let close = capture.events("ws.close");
        assert_eq!(close.len(), 1);
        assert!(close[0]["duration_ms"].is_number());
        assert_eq!(close[0]["route"], "/api/events");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_2xx_logs_debug() {
        let (capture, _guard) = test_capture();
        get_uri("/api/ok", None).await;
        assert_eq!(capture.events("http.request")[0]["level"], "DEBUG");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_500_logs_error_with_error_kind() {
        let (capture, _guard) = test_capture();
        let res = get_uri("/api/boom", None).await;
        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let req = capture.events("http.request");
        assert_eq!(req[0]["level"], "ERROR");
        let err = capture.events("http.error");
        assert_eq!(err.len(), 1, "{:?}", capture.lines());
        assert_eq!(err[0]["level"], "ERROR");
        assert_eq!(err[0]["error_kind"], "io");
    }
}
