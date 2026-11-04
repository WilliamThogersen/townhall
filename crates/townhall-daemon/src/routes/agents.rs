use axum::extract::{Path, State};
use axum::response::Response;
use axum::Json;
use townhall_core::agent::{
    self, Agent, AgentAdapter, ClaudeCodeAdapter, LaunchOptions, ShellAdapter,
};
use serde::{Deserialize, Serialize};

use crate::envelope::{created, ok};
use crate::error::ApiError;
use crate::state::AppState;

const DEFAULT_BASE: &str = "HEAD";

#[derive(Debug, Serialize)]
pub struct AgentView {
    name: String,
    kind: Option<String>,
    branch: String,
    worktree: String,
    alive: bool,
}

impl From<Agent> for AgentView {
    fn from(a: Agent) -> Self {
        Self {
            name: a.name,
            kind: a.kind,
            branch: a.branch,
            worktree: a.worktree.to_string_lossy().into_owned(),
            alive: a.alive,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateAgent {
    name: String,
    base: Option<String>,
    adapter: String,
    prompt: Option<String>,
    command: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SendText {
    text: String,
}

fn adapter_for(kind: &str) -> Result<Box<dyn AgentAdapter>, ApiError> {
    match kind {
        "claude" => Ok(Box::new(ClaudeCodeAdapter)),
        "shell" => Ok(Box::new(ShellAdapter)),
        other => Err(ApiError::BadRequest(format!(
            "unknown adapter `{other}`: expected `claude` or `shell`"
        ))),
    }
}

pub async fn list(State(state): State<AppState>) -> Result<Response, ApiError> {
    let agents: Vec<AgentView> =
        agent::list(state.repo()).await?.into_iter().map(AgentView::from).collect();
    Ok(ok(agents))
}

pub async fn create(
    State(state): State<AppState>,
    Json(req): Json<CreateAgent>,
) -> Result<Response, ApiError> {
    let adapter = adapter_for(&req.adapter)?;
    let base = req.base.as_deref().map(str::trim).filter(|b| !b.is_empty());
    if base.is_some_and(|b| b.starts_with('-')) {
        return Err(ApiError::BadRequest("`base` must not start with `-`".into()));
    }
    let options = LaunchOptions { prompt: req.prompt, command: req.command };
    let started = agent::start(
        state.repo(),
        &req.name,
        base.unwrap_or(DEFAULT_BASE),
        adapter.as_ref(),
        &options,
    )
    .await?;
    Ok(created(AgentView::from(started)))
}

pub async fn remove(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Response, ApiError> {
    agent::stop(state.repo(), &name).await?;
    Ok(ok(name))
}

pub async fn send(
    Path(name): Path<String>,
    Json(req): Json<SendText>,
) -> Result<Response, ApiError> {
    agent::send(&name, &req.text).await?;
    Ok(ok(name))
}

pub async fn output(Path(name): Path<String>) -> Result<Response, ApiError> {
    Ok(ok(agent::output(&name).await?))
}
