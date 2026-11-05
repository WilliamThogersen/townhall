//! Frontend log reports: `POST /api/logs/client` (task T4 of the logging plan, docs/logging.md).
//! A later task adds the `GET` handlers to this file.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use axum::body::Bytes;
use axum::extract::DefaultBodyLimit;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Extension, Json, Router};
use serde::Deserialize;
use serde_json::json;
use townhall_core::redact::redact;

use crate::envelope::{failure, Envelope};
use crate::state::AppState;

/// Request bodies above this are refused with `413`.
pub const MAX_BODY_BYTES: usize = 8 * 1024;
/// Reports accepted per minute, per daemon.
pub const RATE_PER_MINUTE: f64 = 30.0;
/// Events a client may name: `web.*` (browser errors) and `api.*` (failed API calls). Anything
/// else could pass for a daemon event such as `panic` or `task.state`.
const EVENT_PREFIXES: [&str; 2] = ["web.", "api."];
const MAX_EVENT_LEN: usize = 64;

/// One report from the frontend.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Report {
    level: String,
    event: String,
    message: String,
    stack: Option<String>,
    route: Option<String>,
    request_id: Option<String>,
    error_kind: Option<String>,
    status: Option<u16>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Body {
    One(Box<Report>),
    Batch(Vec<Report>),
}

#[derive(Debug, Clone, Copy)]
enum Level {
    Error,
    Warn,
    Info,
    Debug,
}

fn parse_level(level: &str) -> Option<Level> {
    match level.to_ascii_lowercase().as_str() {
        "error" => Some(Level::Error),
        "warn" | "warning" => Some(Level::Warn),
        "info" => Some(Level::Info),
        "debug" => Some(Level::Debug),
        _ => None,
    }
}

fn valid_event(event: &str) -> bool {
    event.len() <= MAX_EVENT_LEN
        && EVENT_PREFIXES.iter().any(|p| event.starts_with(p))
        && event
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'_')
}

/// Token bucket: `RATE_PER_MINUTE` tokens, refilled continuously. `dropped_reported` makes
/// sure one burst of drops logs `web.dropped` once; it resets when a report gets through.
#[derive(Debug)]
pub struct Limiter {
    inner: Mutex<Bucket>,
}

#[derive(Debug)]
struct Bucket {
    tokens: f64,
    last: Instant,
    dropped: u64,
    dropped_reported: bool,
}

/// What `Limiter::take` decided.
#[derive(Debug, PartialEq, Eq)]
pub enum Take {
    Allowed,
    /// Dropped; the first drop of a window also says how many were dropped (always 1 here).
    Dropped {
        first: bool,
    },
}

impl Limiter {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Bucket {
                tokens: RATE_PER_MINUTE,
                last: Instant::now(),
                dropped: 0,
                dropped_reported: false,
            }),
        }
    }

    pub fn take(&self) -> Take {
        self.take_at(Instant::now())
    }

    fn take_at(&self, now: Instant) -> Take {
        let mut b = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        let elapsed = now.saturating_duration_since(b.last).as_secs_f64();
        b.tokens = (b.tokens + elapsed * RATE_PER_MINUTE / 60.0).min(RATE_PER_MINUTE);
        b.last = now;
        if b.tokens >= 1.0 {
            b.tokens -= 1.0;
            b.dropped_reported = false;
            return Take::Allowed;
        }
        b.dropped += 1;
        let first = !b.dropped_reported;
        b.dropped_reported = true;
        Take::Dropped { first }
    }
}

impl Default for Limiter {
    fn default() -> Self {
        Self::new()
    }
}

/// The routes of this file. The limiter lives in the router, so each daemon has its own.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/logs/client", post(client))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .layer(Extension(Arc::new(Limiter::new())))
}

/// `202` once the reports are logged or dropped by the rate limit; `400` for bad JSON, an
/// unknown level or a disallowed event (nothing from that body is logged); `413` over 8 KB.
async fn client(Extension(limiter): Extension<Arc<Limiter>>, body: Bytes) -> Response {
    let reports = match serde_json::from_slice::<Body>(&body) {
        Ok(Body::One(r)) => vec![*r],
        Ok(Body::Batch(rs)) => rs,
        Err(e) => return failure(StatusCode::BAD_REQUEST, format!("invalid log report: {e}")),
    };
    let mut levels = Vec::with_capacity(reports.len());
    for r in &reports {
        let Some(level) = parse_level(&r.level) else {
            return failure(
                StatusCode::BAD_REQUEST,
                format!("unknown level: {}", r.level),
            );
        };
        if !valid_event(&r.event) {
            return failure(
                StatusCode::BAD_REQUEST,
                "event must be web.* or api.*".into(),
            );
        }
        levels.push(level);
    }

    let mut accepted = 0;
    for (report, level) in reports.iter().zip(levels) {
        match limiter.take() {
            Take::Allowed => {
                emit(report, level);
                accepted += 1;
            }
            Take::Dropped { first } => {
                if first {
                    tracing::warn!(
                        target: "townhall::web",
                        event = "web.dropped",
                        limit_per_minute = RATE_PER_MINUTE as u64,
                        "client log reports dropped by the rate limit"
                    );
                }
            }
        }
    }
    let envelope = Envelope {
        success: true,
        data: Some(json!({ "accepted": accepted, "dropped": reports.len() - accepted })),
        error: None,
    };
    (StatusCode::ACCEPTED, Json(envelope)).into_response()
}

/// Re-emits `r` as a `townhall::web` event. Every string is redacted (and cut to 500 chars).
fn emit(r: &Report, level: Level) {
    let message = redact(&r.message);
    let stack = r.stack.as_deref().map(redact);
    let route = r.route.as_deref().map(redact);
    let request_id = r.request_id.as_deref().map(redact);
    let error_kind = r.error_kind.as_deref().map(redact);
    let (stack, route) = (stack.as_deref(), route.as_deref());
    let (request_id, error_kind) = (request_id.as_deref(), error_kind.as_deref());
    macro_rules! log {
        ($mac:ident) => {
            tracing::$mac!(
                target: "townhall::web",
                event = r.event.as_str(),
                stack,
                route,
                request_id,
                error_kind,
                status = r.status,
                "{}",
                message
            )
        };
    }
    match level {
        Level::Error => log!(error),
        Level::Warn => log!(warn),
        Level::Info => log!(info),
        Level::Debug => log!(debug),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn bucket_refills_over_time() {
        let limiter = Limiter::new();
        let t0 = Instant::now();
        for _ in 0..30 {
            assert_eq!(limiter.take_at(t0), Take::Allowed);
        }
        assert_eq!(limiter.take_at(t0), Take::Dropped { first: true });
        assert_eq!(limiter.take_at(t0), Take::Dropped { first: false });
        // Two seconds earns one token back, which also opens a new drop window.
        let t1 = t0 + Duration::from_secs(2);
        assert_eq!(limiter.take_at(t1), Take::Allowed);
        assert_eq!(limiter.take_at(t1), Take::Dropped { first: true });
    }

    #[test]
    fn events_are_limited_to_web_and_api() {
        assert!(valid_event("web.unhandled_rejection"));
        assert!(valid_event("api.http"));
        assert!(!valid_event("panic"));
        assert!(!valid_event("task.state"));
        assert!(!valid_event("web.Bad Event"));
    }
}
