use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use townhall_core::redact::redact;
use townhall_core::Error;

use crate::envelope::failure;

pub const NOT_IMPLEMENTED: &str = "not implemented yet";

/// API error: a townhall-core error, a boundary validation failure, or a route
/// that a later unit has not built yet.
#[derive(Debug)]
pub enum ApiError {
    Core(Error),
    BadRequest(String),
    Unauthorized(String),
    Forbidden(String),
    NotImplemented,
}

impl From<Error> for ApiError {
    fn from(e: Error) -> Self {
        Self::Core(e)
    }
}

pub fn status_for(e: &Error) -> StatusCode {
    match e {
        Error::InvalidName(_) | Error::InvalidInput(_) => StatusCode::BAD_REQUEST,
        Error::NotFound(_) | Error::Missing { .. } => StatusCode::NOT_FOUND,
        Error::Conflict(_) => StatusCode::CONFLICT,
        Error::TooLarge(_) => StatusCode::PAYLOAD_TOO_LARGE,
        Error::Unavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            Self::BadRequest(msg) => failure(StatusCode::BAD_REQUEST, msg),
            Self::Unauthorized(msg) => failure(StatusCode::UNAUTHORIZED, msg),
            Self::Forbidden(msg) => failure(StatusCode::FORBIDDEN, msg),
            Self::NotImplemented => failure(StatusCode::NOT_IMPLEMENTED, NOT_IMPLEMENTED.into()),
            Self::Core(e) => {
                let status = status_for(&e);
                if status.is_server_error() {
                    tracing::error!(
                        target: "townhall::http",
                        event = "http.error",
                        status = status.as_u16(),
                        error_kind = e.kind(),
                        error = %redact(&e.to_string()),
                        "internal error"
                    );
                }
                failure(status, e.to_string())
            }
        }
    }
}
