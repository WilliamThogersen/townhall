//! `/api/repos`: the repo registry (the `repos` table, through `RepoRegistry`).

use axum::extract::{Path, Query, State};
use axum::response::Response;
use serde::Deserialize;
use serde_json::json;
use townhall_core::fleet;
use townhall_core::model::{AddRepo, PatchRepo, PutRepoAccounts};
use townhall_core::plugins::hooks;

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
    let repo = ctx.registry()?.add(&req).await?;
    hooks::fire(ctx, "repo.added", json!({"repo": repo}));
    Ok(created(repo))
}

/// Body `{"name"?, "baseBranch"?, "path"?, "moveFolder"?}`: renames the repo, changes its base
/// branch, points it at a new path of the same git repo, or (`moveFolder`) moves its folder.
pub async fn patch(
    State(state): State<AppState>,
    Path(name): Path<String>,
    JsonBody(body): JsonBody<PatchRepo>,
) -> Result<Response, ApiError> {
    let ctx = state.ctx();
    ctx.ensure_startup_repo().await;
    Ok(ok(fleet::edit(ctx, &name, &body).await?))
}

/// Body `{"githubAccount"?: login | null, "gitlabAccount"?: login | null}`: the registered
/// account this repo uses on each forge. A missing key leaves its pick, `null` clears it.
pub async fn put_accounts(
    State(state): State<AppState>,
    Path(name): Path<String>,
    JsonBody(body): JsonBody<PutRepoAccounts>,
) -> Result<Response, ApiError> {
    let ctx = state.ctx();
    ctx.ensure_startup_repo().await;
    Ok(ok(ctx.store()?.set_repo_accounts(&name, body).await?))
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
