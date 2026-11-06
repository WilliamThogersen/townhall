//! Proof artifacts of a task: the list for the web app and the bytes of one. Only what the
//! store typed as a raster image or as plain text or markdown is `inline`; anything else is a
//! download. `nosniff` and a sandbox policy keep a browser from running any of it.

use std::path::Path as FsPath;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::header::{
    CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_SECURITY_POLICY, CONTENT_TYPE,
    X_CONTENT_TYPE_OPTIONS,
};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use townhall_core::files::mime::UNKNOWN;
use townhall_core::files::naming::disposition;
use townhall_core::model::{Artifact, MAX_ARTIFACT_BYTES};
use townhall_core::Error;

use crate::envelope::{failure, ok};
use crate::error::ApiError;
use crate::state::AppState;

fn gone(a: &Artifact) -> Response {
    let message = format!("the file of artifact `{}` is gone", a.id);
    failure(StatusCode::NOT_FOUND, message)
}

async fn is_own_file(path: &FsPath) -> std::io::Result<bool> {
    let Some(folder) = path.parent() else {
        return Ok(false);
    };
    let meta = tokio::fs::symlink_metadata(path).await?;
    if !meta.is_file() || meta.len() > MAX_ARTIFACT_BYTES {
        return Ok(false);
    }
    let real = tokio::fs::canonicalize(path).await?;
    let real_folder = tokio::fs::canonicalize(folder).await?;
    Ok(real.parent() == Some(real_folder.as_path()))
}

/// `Artifact[]` of the task, oldest first.
pub async fn list(
    State(state): State<AppState>,
    Path(task): Path<String>,
) -> Result<Response, ApiError> {
    Ok(ok(state.ctx().store()?.artifacts(&task).await?))
}

/// The bytes with their type, length and a safe file name; never the envelope.
pub async fn download(
    State(state): State<AppState>,
    Path((task, id)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    let (record, path) = state.ctx().store()?.artifact(&task, &id).await?;
    match is_own_file(&path).await {
        Ok(true) => {}
        Ok(false) => return Ok(gone(&record)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(gone(&record)),
        Err(e) => return Err(Error::io(format!("read {}", path.display()), e).into()),
    }
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|e| Error::io(format!("read {}", path.display()), e))?;
    let inline = record.shows_inline();
    let mime = match inline {
        true => HeaderValue::from_str(&record.mime).unwrap_or(HeaderValue::from_static(UNKNOWN)),
        false => HeaderValue::from_static(UNKNOWN),
    };
    let named = HeaderValue::from_str(&disposition(inline, &record.name))
        .unwrap_or(HeaderValue::from_static("attachment"));
    let headers = [
        (CONTENT_TYPE, mime),
        (CONTENT_LENGTH, HeaderValue::from(bytes.len())),
        (CONTENT_DISPOSITION, named),
        (X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff")),
        (
            CONTENT_SECURITY_POLICY,
            HeaderValue::from_static("sandbox; default-src 'none'"),
        ),
    ];
    Ok((StatusCode::OK, headers, Body::from(bytes)).into_response())
}
