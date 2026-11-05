//! Conversations (unit 2.2): list, create, detail, patch, send, stop, end. See `docs/daemon-api.md`.
//! The work happens in `townhall_core::runner`; these handlers only extract and wrap.

use axum::extract::multipart::Multipart;
use axum::extract::{FromRequest, Path, Request, State};
use axum::http::header::CONTENT_TYPE;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use townhall_core::model::{ConversationPatch, NewMessage};
use townhall_core::runner;

use super::attachments;
use super::events::FOREIGN_ORIGIN;
use crate::cors::origin_allowed;
use crate::envelope::{created, failure, ok};
use crate::error::ApiError;
use crate::extract::JsonBody;
use crate::state::AppState;

/// Newest activity first.
pub async fn list(State(state): State<AppState>) -> Result<Response, ApiError> {
    Ok(ok(state.ctx().store()?.conversations().await?))
}

fn is_multipart(headers: &HeaderMap) -> bool {
    headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.trim_start().starts_with("multipart/form-data"))
}

async fn create_with_files(state: &AppState, request: Request) -> Result<Response, ApiError> {
    if !origin_allowed(request.headers()) {
        return Ok(failure(StatusCode::FORBIDDEN, FOREIGN_ORIGIN.into()));
    }
    let body = Multipart::from_request(request, state)
        .await
        .map_err(|_| ApiError::BadRequest(attachments::NOT_MULTIPART.into()))?;
    let (text, files) = attachments::start_files(body).await?;
    Ok(created(
        runner::create_with_files(state.ctx(), text, files).await?,
    ))
}

pub async fn create(State(state): State<AppState>, request: Request) -> Result<Response, ApiError> {
    if is_multipart(request.headers()) {
        return create_with_files(&state, request).await;
    }
    let JsonBody(message) = JsonBody::<NewMessage>::from_request(request, &state).await?;
    Ok(created(runner::create(state.ctx(), message).await?))
}

pub async fn detail(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    Ok(ok(state.ctx().store()?.detail(&id).await?))
}

pub async fn patch(
    State(state): State<AppState>,
    Path(id): Path<String>,
    JsonBody(change): JsonBody<ConversationPatch>,
) -> Result<Response, ApiError> {
    Ok(ok(runner::patch(state.ctx(), &id, change).await?))
}

/// Your `ThreadEntry`. Clears `ended`; `409` while a turn runs.
pub async fn send(
    State(state): State<AppState>,
    Path(id): Path<String>,
    JsonBody(message): JsonBody<NewMessage>,
) -> Result<Response, ApiError> {
    Ok(ok(runner::send(state.ctx(), &id, message).await?))
}

pub async fn stop(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    Ok(ok(runner::stop(state.ctx(), &id).await?))
}

/// `ended: true`; its tasks keep running.
pub async fn end(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    Ok(ok(runner::end(state.ctx(), &id).await?))
}
