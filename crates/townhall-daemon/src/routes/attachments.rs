//! Attachments (unit 2.6): upload one file per request, list, download.
//! Not built yet: every handler answers `501 not implemented yet`.

use crate::error::ApiError;

pub async fn list() -> ApiError {
    ApiError::NotImplemented
}

pub async fn upload() -> ApiError {
    ApiError::NotImplemented
}

pub async fn download() -> ApiError {
    ApiError::NotImplemented
}
