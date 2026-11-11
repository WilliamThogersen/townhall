//! Workflow runs (unit 2.8c): start a run, read its state.
//! `start` is not built yet and answers `501 not implemented yet`.

use axum::extract::{Path, Query, State};
use axum::response::Response;
use serde::{Deserialize, Serialize};
use townhall_core::definitions::parse_workflow;
use townhall_core::store::{Run, RunStep};
use townhall_core::Error;

use crate::envelope::ok;
use crate::error::ApiError;
use crate::state::AppState;

const PENDING: &str = "pending";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StepView {
    id: String,
    agent: String,
    state: String,
    round: u32,
    task: Option<String>,
    verdict: Option<String>,
    note: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RunView {
    id: String,
    workflow: String,
    conversation_id: String,
    repo: String,
    base_branch: String,
    state: String,
    input: String,
    steps: Vec<StepView>,
}

#[derive(Debug, Deserialize)]
pub struct RunsQuery {
    conversation: Option<String>,
}

fn newest<'a>(rows: &'a [RunStep], id: &str) -> Option<&'a RunStep> {
    rows.iter()
        .filter(|row| row.step_id == id)
        .max_by_key(|row| row.round)
}

async fn view(state: &AppState, run: Run) -> Result<RunView, ApiError> {
    let rows = state.ctx().store()?.run_steps(&run.id).await?;
    let def = parse_workflow(&run.snapshot)
        .map_err(|e| Error::Corrupt {
            path: format!("townhall.db runs.snapshot of `{}`", run.id),
            message: e.to_string(),
        })?
        .def;
    let steps = def
        .steps
        .iter()
        .map(|step| match newest(&rows, &step.id) {
            Some(row) => StepView {
                id: step.id.clone(),
                agent: step.agent.clone(),
                state: row.state.clone(),
                round: row.round,
                task: row.task.clone(),
                verdict: row.verdict.clone(),
                note: row.note.clone(),
            },
            None => StepView {
                id: step.id.clone(),
                agent: step.agent.clone(),
                state: PENDING.into(),
                round: 0,
                task: None,
                verdict: None,
                note: None,
            },
        })
        .collect();
    Ok(RunView {
        id: run.id,
        workflow: run.workflow,
        conversation_id: run.conversation_id,
        repo: run.repo,
        base_branch: run.base_branch,
        state: run.state,
        input: run.input,
        steps,
    })
}

pub async fn start() -> ApiError {
    ApiError::NotImplemented
}

pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    match state.ctx().store()?.run(&id).await? {
        Some(run) => Ok(ok(view(&state, run).await?)),
        None => Err(Error::missing("run", id).into()),
    }
}

pub async fn list(
    State(state): State<AppState>,
    Query(q): Query<RunsQuery>,
) -> Result<Response, ApiError> {
    let conversation = q
        .conversation
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .ok_or_else(|| ApiError::BadRequest("`conversation` is required".into()))?;
    let mut views = vec![];
    for run in state.ctx().store()?.runs_of(conversation).await? {
        views.push(view(&state, run).await?);
    }
    Ok(ok(views))
}
