//! `/api/repos`: the repo registry in `config.json`.

use axum::extract::{Path, State};
use axum::response::Response;
use townhall_core::fleet;
use townhall_core::model::AddRepo;

use crate::envelope::{created, ok};
use crate::error::ApiError;
use crate::extract::JsonBody;
use crate::state::AppState;

pub async fn list(State(state): State<AppState>) -> Result<Response, ApiError> {
    Ok(ok(state.ctx().repos().await?))
}

pub async fn add(
    State(state): State<AppState>,
    JsonBody(req): JsonBody<AddRepo>,
) -> Result<Response, ApiError> {
    let ctx = state.ctx();
    ctx.ensure_startup_repo().await;
    Ok(created(ctx.registry()?.add(&req).await?))
}

/// Unregisters only: the folder, its worktrees and branches stay on disk.
pub async fn remove(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Response, ApiError> {
    Ok(ok(fleet::unregister(state.ctx(), &name).await?))
}
