//! A download: only the regular file directly inside the attachment's own folder, with its
//! type, length and name. Images the bytes back up are `inline`; everything else is an
//! `attachment`, and `nosniff` plus a sandbox policy keep a browser from running any of it.

use std::path::{Component, Path};

use axum::body::Body;
use axum::http::header::{
    CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_SECURITY_POLICY, CONTENT_TYPE,
    X_CONTENT_TYPE_OPTIONS,
};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use townhall_core::model::{Attachment, AttachmentKind, MAX_ATTACHMENT_BYTES};
use townhall_core::Error;

use crate::envelope::failure;
use crate::error::ApiError;
use townhall_core::files::mime::{image_extensions, UNKNOWN};
use townhall_core::files::naming::disposition;

fn gone(a: &Attachment) -> Response {
    let message = format!("the file of attachment `{}` is gone", a.id);
    failure(StatusCode::NOT_FOUND, message)
}

/// True when `path` is `<folder>/<name>`, `name` a single plain part, and the file is a
/// regular file whose real folder is the attachment folder's real path.
async fn inside_folder(a: &Attachment, path: &Path) -> std::io::Result<bool> {
    let mut parts = Path::new(&a.name).components();
    let single = matches!(parts.next(), Some(Component::Normal(_))) && parts.next().is_none();
    let (Some(folder), Some(file_name)) = (path.parent(), path.file_name()) else {
        return Ok(false);
    };
    if !single || file_name != a.name.as_str() {
        return Ok(false);
    }
    let meta = tokio::fs::symlink_metadata(path).await?;
    if !meta.is_file() || meta.len() > MAX_ATTACHMENT_BYTES {
        return Ok(false);
    }
    let real = tokio::fs::canonicalize(path).await?;
    let real_folder = tokio::fs::canonicalize(folder).await?;
    Ok(real.parent() == Some(real_folder.as_path()))
}

fn shows_inline(a: &Attachment) -> bool {
    a.kind == AttachmentKind::Image && image_extensions(&a.mime).is_some()
}

/// The bytes of `a`, read from `path` (`Store::attachment`'s), with safe headers.
pub async fn file(a: &Attachment, path: &Path) -> Result<Response, ApiError> {
    match inside_folder(a, path).await {
        Ok(true) => {}
        Ok(false) => return Ok(gone(a)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(gone(a)),
        Err(e) => return Err(Error::io(format!("read {}", path.display()), e).into()),
    }
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|e| Error::io(format!("read {}", path.display()), e))?;
    let mime = HeaderValue::from_str(&a.mime).unwrap_or(HeaderValue::from_static(UNKNOWN));
    let named = HeaderValue::from_str(&disposition(shows_inline(a), &a.name))
        .unwrap_or(HeaderValue::from_static("attachment"));
    let headers = [
        (CONTENT_TYPE, mime),
        (CONTENT_LENGTH, HeaderValue::from(bytes.len())),
        (CONTENT_DISPOSITION, named),
        (X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff")),
        (CONTENT_SECURITY_POLICY, HeaderValue::from_static("sandbox")),
    ];
    Ok((StatusCode::OK, headers, Body::from(bytes)).into_response())
}
