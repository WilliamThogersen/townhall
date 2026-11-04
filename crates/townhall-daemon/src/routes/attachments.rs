//! Attachments (unit 2.6): upload one file per request, list, download. See `docs/daemon-api.md`.
//! An upload checks the origin first (CORS does not cover multipart), reads the one `file`
//! field while counting its bytes, takes the type from the bytes and cleans the name; the
//! store writes `conversations/<id>/files/<attachment>/<name>`.

mod form;
mod mime;
mod naming;
mod serve;

use axum::extract::multipart::{Multipart, MultipartRejection};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;

use super::events::FOREIGN_ORIGIN;
use crate::cors::origin_allowed;
use crate::envelope::{created, failure, ok};
use crate::error::ApiError;
use crate::state::AppState;

const NOT_MULTIPART: &str = "send the file as multipart/form-data";

/// `Attachment[]` in upload order.
pub async fn list(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    Ok(ok(state.ctx().store()?.attachments(&id).await?))
}

/// `201 Attachment`. `403` foreign origin, `404` unknown conversation, `400` no or several
/// files, an empty file or a broken body, `413` over 20 MB.
pub async fn upload(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Result<Multipart, MultipartRejection>,
) -> Result<Response, ApiError> {
    if !origin_allowed(&headers) {
        return Ok(failure(StatusCode::FORBIDDEN, FOREIGN_ORIGIN.into()));
    }
    let store = state.ctx().store()?;
    store.conversation(&id).await?;
    let body = body.map_err(|_| ApiError::BadRequest(NOT_MULTIPART.into()))?;
    let file = form::read(body).await?;
    let mime = mime::detect(&file.bytes, file.content_type.as_deref(), &file.name);
    let name = naming::file_name(&file.name, mime::image_extensions(&mime));
    let saved = store.save_attachment(&id, &name, &mime, file.bytes).await?;
    Ok(created(saved))
}

/// The bytes with their type, length and a safe file name; never the envelope.
pub async fn download(
    State(state): State<AppState>,
    Path((id, attachment)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    let (record, path) = state.ctx().store()?.attachment(&id, &attachment).await?;
    serve::file(&record, &path).await
}
