//! `/api/agents`: agents across every registered repo. A name is resolved across repos;
//! `?repo=<name>` picks one when two repos have it.

use axum::extract::{Path, Query, State};
use axum::response::Response;
use serde::Deserialize;
use townhall_core::agent::{
    self, state, AgentAdapter, ClaudeCodeAdapter, LaunchOptions, ShellAdapter,
};
use townhall_core::cmd::validate_name;
use townhall_core::model::{AgentEvent, AgentInfo, EntryBody};
use townhall_core::{fleet, Context, Error};

use crate::envelope::{created, ok};
use crate::error::ApiError;
use crate::extract::JsonBody;
use crate::state::AppState;

const DEFAULT_BASE: &str = "HEAD";

/// `POST /api/agents` body. There is no permission field: the mode always comes from settings.
#[derive(Debug, Deserialize)]
pub struct CreateAgent {
    name: String,
    base: Option<String>,
    adapter: String,
    prompt: Option<String>,
    command: Option<String>,
    /// Registered repo name; the startup repo when missing.
    repo: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SendText {
    text: String,
}

/// `?repo=<name>` on every `/api/agents/{name}...` route.
#[derive(Debug, Default, Deserialize)]
pub struct RepoQuery {
    pub repo: Option<String>,
}

fn adapter_for(kind: &str, ctx: &Context) -> Result<Box<dyn AgentAdapter>, ApiError> {
    match kind {
        "claude" => Ok(Box::new(ClaudeCodeAdapter::new(ctx.claude_bin()))),
        "shell" => Ok(Box::new(ShellAdapter)),
        other => Err(ApiError::BadRequest(format!(
            "unknown adapter `{other}`: expected `claude` or `shell`"
        ))),
    }
}

pub async fn list(State(state): State<AppState>) -> Result<Response, ApiError> {
    Ok(ok(fleet::list(state.ctx()).await?))
}

async fn target(ctx: &Context, repo: Option<&str>) -> Result<fleet::Located, ApiError> {
    match repo.map(str::trim).filter(|r| !r.is_empty()) {
        Some(name) => {
            let found = ctx.repo(name).await?;
            Ok(fleet::Located {
                workspace: ctx.workspace(&found)?,
                repo: found.name,
            })
        }
        None => Ok(fleet::Located {
            repo: ctx.startup_repo_name().await,
            workspace: ctx.startup().clone(),
        }),
    }
}

pub async fn create(
    State(state): State<AppState>,
    JsonBody(req): JsonBody<CreateAgent>,
) -> Result<Response, ApiError> {
    let ctx = state.ctx();
    let adapter = adapter_for(&req.adapter, ctx)?;
    let base = req.base.as_deref().map(str::trim).filter(|b| !b.is_empty());
    if base.is_some_and(|b| b.starts_with('-')) {
        return Err(ApiError::BadRequest(
            "`base` must not start with `-`".into(),
        ));
    }
    validate_name(&req.name)?;
    if let Some(taken) = fleet::holders(ctx, &req.name).await?.first() {
        return Err(Error::Conflict(format!(
            "an agent named `{}` already exists in repo `{}`",
            req.name, taken.repo
        ))
        .into());
    }
    let at = target(ctx, req.repo.as_deref()).await?;
    let options = LaunchOptions {
        prompt: req.prompt,
        command: req.command,
        permission_mode: ctx.settings().await?.permission_mode,
        ..LaunchOptions::default()
    };
    let base = base.unwrap_or(DEFAULT_BASE);
    let started = agent::start(&at.workspace, &req.name, base, adapter.as_ref(), &options).await?;
    let now = state::detect(&at.workspace, &started).await;
    Ok(created(AgentInfo::new(started, &at.repo, now)))
}

pub async fn remove(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(q): Query<RepoQuery>,
) -> Result<Response, ApiError> {
    let at = fleet::locate(state.ctx(), &name, q.repo.as_deref()).await?;
    agent::stop(&at.workspace, &name).await?;
    Ok(ok(name))
}

pub async fn send(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(q): Query<RepoQuery>,
    JsonBody(req): JsonBody<SendText>,
) -> Result<Response, ApiError> {
    let ctx = state.ctx();
    let at = fleet::locate(ctx, &name, q.repo.as_deref()).await?;
    agent::send(&at.workspace, &name, &req.text).await?;
    note_sent(ctx, &name, &req.text).await;
    Ok(ok(name))
}

/// A task's agent got text: add `→ sent to <agent>` to its conversation. The text was
/// already sent, so a failure here is logged rather than failing the request.
async fn note_sent(ctx: &Context, name: &str, text: &str) {
    let noted = async {
        let store = ctx.store()?;
        match store.find_task(name).await? {
            Some(task) => {
                let body = EntryBody::Agent {
                    agent: task.name,
                    event: AgentEvent::Sent,
                    text: text.to_string(),
                };
                store
                    .append_entry(&task.conversation_id, body)
                    .await
                    .map(|_| ())
            }
            None => Ok(()),
        }
    };
    if let Err(e) = noted.await {
        eprintln!("warn: sent to `{name}` but not noted in its conversation: {e}");
    }
}

pub async fn output(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(q): Query<RepoQuery>,
) -> Result<Response, ApiError> {
    let at = fleet::locate(state.ctx(), &name, q.repo.as_deref()).await?;
    Ok(ok(agent::output(&at.workspace, &name).await?))
}
