//! `GET /api/agents/{name}/steps` (unit 2.7): the tool calls in the agent's Claude
//! transcript. Transcripts come from `$HOME/.claude/projects`, or `TOWNHALL_CLAUDE_PROJECTS`.

use axum::extract::{Path, Query, State};
use axum::response::Response;
use townhall_core::{fleet, steps, worktree, Error};

use super::agents::RepoQuery;
use crate::envelope::ok;
use crate::error::ApiError;
use crate::state::AppState;

/// `Step[]` of the agent's newest transcript since its worktree was made (a reused name never
/// shows the earlier agent's steps); `[]` without one. `404` when the repo has no such agent.
pub async fn list(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(q): Query<RepoQuery>,
) -> Result<Response, ApiError> {
    let at = fleet::locate(state.ctx(), &name, q.repo.as_deref()).await?;
    let found = worktree::list(&at.workspace)
        .await?
        .into_iter()
        .find(|wt| wt.name == name);
    let Some(wt) = found else {
        return Err(Error::NotFound(name).into());
    };
    let projects = steps::claude_projects_dir()?;
    Ok(ok(steps::steps_for_agent(&projects, &wt.path).await?))
}
