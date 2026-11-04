use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use castle_core::Error;

use crate::envelope::failure;

/// API error: a castle-core error or a boundary validation failure.
#[derive(Debug)]
pub enum ApiError {
    Core(Error),
    BadRequest(String),
}

impl From<Error> for ApiError {
    fn from(e: Error) -> Self {
        Self::Core(e)
    }
}

pub fn status_for(e: &Error) -> StatusCode {
    match e {
        Error::InvalidName(_) | Error::InvalidInput(_) => StatusCode::BAD_REQUEST,
        Error::NotFound(_) => StatusCode::NOT_FOUND,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            Self::BadRequest(msg) => failure(StatusCode::BAD_REQUEST, msg),
            Self::Core(e) => {
                let status = status_for(&e);
                if status == StatusCode::INTERNAL_SERVER_ERROR {
                    eprintln!("internal error: {e}");
                }
                failure(status, e.to_string())
            }
        }
    }
}
