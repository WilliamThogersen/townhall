//! The `/api/events` websocket (unit 2.2): forwards the event bus as JSON messages.
//! Not built yet: every handler answers `501 not implemented yet`.

use crate::error::ApiError;

pub async fn socket() -> ApiError {
    ApiError::NotImplemented
}
