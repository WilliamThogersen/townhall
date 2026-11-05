//! Reads the one `file` field of an upload, counting bytes as they arrive: past 20 MB it
//! stops with `413`, so a body is never held beyond that.

use axum::extract::multipart::{Field, Multipart, MultipartError};
use axum::http::StatusCode;
use townhall_core::model::MAX_ATTACHMENT_BYTES;
use townhall_core::Error;

use super::naming;
use crate::error::ApiError;

const FIELD: &str = "file";
const TEXT: &str = "text";
const START_FIELDS: &str = "send a `text` field and any number of `file` fields";
const ONE_FILE: &str = "send one file in a field named `file`";
const BROKEN: &str = "the upload is not valid multipart form data";
const TOO_LARGE: &str = "the upload is over 20 MB";

/// The file as sent: its raw name (cleaned later), the part's content type and the bytes.
#[derive(Debug)]
pub struct Upload {
    pub name: String,
    pub content_type: Option<String>,
    pub bytes: Vec<u8>,
}

fn broken(e: MultipartError) -> ApiError {
    if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
        ApiError::Core(Error::TooLarge(TOO_LARGE.into()))
    } else {
        ApiError::BadRequest(BROKEN.into())
    }
}

fn limit() -> usize {
    usize::try_from(MAX_ATTACHMENT_BYTES).unwrap_or(usize::MAX)
}

/// A field without a file name is a text value (`-F file=hello`), not a file: `400`.
async fn read_field(mut field: Field<'_>) -> Result<Upload, ApiError> {
    let Some(name) = field.file_name().map(str::to_string) else {
        return Err(ApiError::BadRequest(ONE_FILE.into()));
    };
    let content_type = field.content_type().map(str::to_string);
    let mut bytes = Vec::new();
    while let Some(chunk) = field.chunk().await.map_err(broken)? {
        if bytes.len() + chunk.len() > limit() {
            let shown = naming::file_name(&name, None);
            return Err(ApiError::Core(Error::TooLarge(format!(
                "{shown} is over 20 MB"
            ))));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(Upload {
        name,
        content_type,
        bytes,
    })
}

/// The one non-empty file in the `file` field. Any other field, a `file` field that is
/// not a file, a second file or none is `400`.
pub async fn read(mut form: Multipart) -> Result<Upload, ApiError> {
    let mut found = None;
    while let Some(field) = form.next_field().await.map_err(broken)? {
        if field.name() != Some(FIELD) || found.is_some() {
            return Err(ApiError::BadRequest(ONE_FILE.into()));
        }
        found = Some(read_field(field).await?);
    }
    non_empty(found.ok_or_else(|| ApiError::BadRequest(ONE_FILE.into()))?)
}

pub async fn read_start(mut form: Multipart) -> Result<(String, Vec<Upload>), ApiError> {
    let mut text = String::new();
    let mut files = Vec::new();
    while let Some(field) = form.next_field().await.map_err(broken)? {
        match field.name() {
            Some(FIELD) => files.push(non_empty(read_field(field).await?)?),
            Some(TEXT) => text = field.text().await.map_err(broken)?,
            _ => return Err(ApiError::BadRequest(START_FIELDS.into())),
        }
    }
    Ok((text, files))
}

fn non_empty(upload: Upload) -> Result<Upload, ApiError> {
    if upload.bytes.is_empty() {
        let shown = naming::file_name(&upload.name, None);
        return Err(ApiError::BadRequest(format!("{shown} is empty")));
    }
    Ok(upload)
}
