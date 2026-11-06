//! `/api/repos`: the repo registry (the `repos` table, through `RepoRegistry`).

use axum::extract::{Path, Query, State};
use axum::response::Response;
use serde::Deserialize;
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

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveQuery {
    #[serde(default)]
    delete_clone: bool,
}

/// Unregisters: the folder, its worktrees and branches stay on disk, unless `?deleteClone=true`
/// asks to delete a clone townhall made.
pub async fn remove(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(query): Query<RemoveQuery>,
) -> Result<Response, ApiError> {
    Ok(ok(
        fleet::remove(state.ctx(), &name, query.delete_clone).await?
    ))
}
