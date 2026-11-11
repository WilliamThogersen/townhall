//! Tasks (unit 2.3): list and get. Tasks are created by townhall's session through MCP.

use axum::extract::rejection::QueryRejection;
use axum::extract::{Path, Query, State};
use axum::response::Response;
use serde::Deserialize;
use serde_json::Value;
use townhall_core::model::{Task, TaskState};
use townhall_core::Error;

use crate::envelope::ok;
use crate::error::ApiError;
use crate::state::AppState;

const STATES: &str = "queued, working, needsYou, inCi, inReview, merged, ended, error";

/// `GET /api/tasks` filters. A blank value filters nothing.
#[derive(Debug, Default, Deserialize)]
pub struct TaskQuery {
    conversation: Option<String>,
    repo: Option<String>,
    state: Option<String>,
}

fn given(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|v| !v.is_empty())
}

fn parse_state(raw: &str) -> Result<TaskState, ApiError> {
    serde_json::from_value(Value::String(raw.to_string())).map_err(|_| {
        ApiError::BadRequest(format!("unknown state `{raw}`: expected one of {STATES}"))
    })
}

fn keep(task: &Task, q: &TaskQuery, state: Option<TaskState>) -> bool {
    given(q.conversation.as_deref()).is_none_or(|c| task.conversation_id == c)
        && given(q.repo.as_deref()).is_none_or(|r| task.repo.eq_ignore_ascii_case(r))
        && state.is_none_or(|s| task.state == s)
}

pub(super) async fn select(
    state: &AppState,
    query: Result<Query<TaskQuery>, QueryRejection>,
) -> Result<Vec<Task>, ApiError> {
    let Query(q) = query.map_err(|e| ApiError::BadRequest(e.body_text()))?;
    let wanted = given(q.state.as_deref()).map(parse_state).transpose()?;
    let tasks = state.ctx().store()?.all_tasks().await?;
    Ok(tasks.into_iter().filter(|t| keep(t, &q, wanted)).collect())
}

/// `GET /api/tasks?conversation=&repo=&state=`: every matching task, oldest first.
pub async fn list(
    State(state): State<AppState>,
    query: Result<Query<TaskQuery>, QueryRejection>,
) -> Result<Response, ApiError> {
    Ok(ok(select(&state, query).await?))
}

/// `GET /api/tasks/{name}`, ignoring case.
pub async fn get(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Response, ApiError> {
    match state.ctx().store()?.find_task(&name).await? {
        Some(task) => Ok(ok(task)),
        None => Err(Error::missing("task", name).into()),
    }
}
