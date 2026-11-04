//! Conversations (unit 2.2): list, create, detail, patch, send, stop, end. See `docs/daemon-api.md`.
//! The work happens in `townhall_core::runner`; these handlers only extract and wrap.

use axum::extract::{Path, State};
use axum::response::Response;
use townhall_core::model::{ConversationPatch, NewMessage};
use townhall_core::runner;

use crate::envelope::{created, ok};
use crate::error::ApiError;
use crate::extract::JsonBody;
use crate::state::AppState;

/// Newest activity first.
pub async fn list(State(state): State<AppState>) -> Result<Response, ApiError> {
    Ok(ok(state.ctx().store()?.conversations().await?))
}

/// `201 ConversationDetail`: created, your message appended, the first turn started.
pub async fn create(
    State(state): State<AppState>,
    JsonBody(message): JsonBody<NewMessage>,
) -> Result<Response, ApiError> {
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
