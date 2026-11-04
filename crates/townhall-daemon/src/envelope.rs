use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

/// Every API response: `{success, data, error}`.
#[derive(Debug, Serialize)]
pub struct Envelope<T: Serialize> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

pub fn ok<T: Serialize>(data: T) -> Response {
    reply(
        StatusCode::OK,
        Envelope {
            success: true,
            data: Some(data),
            error: None,
        },
    )
}

pub fn created<T: Serialize>(data: T) -> Response {
    reply(
        StatusCode::CREATED,
        Envelope {
            success: true,
            data: Some(data),
            error: None,
        },
    )
}

pub fn failure(status: StatusCode, message: String) -> Response {
    reply(
        status,
        Envelope::<()> {
            success: false,
            data: None,
            error: Some(message),
        },
    )
}

fn reply<T: Serialize>(status: StatusCode, body: Envelope<T>) -> Response {
    (status, Json(body)).into_response()
}
