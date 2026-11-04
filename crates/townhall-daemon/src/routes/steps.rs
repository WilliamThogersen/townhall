//! Steps per agent (unit 2.7), from Claude's transcript.
//! Not built yet: every handler answers `501 not implemented yet`.

use crate::error::ApiError;

pub async fn list() -> ApiError {
    ApiError::NotImplemented
}
