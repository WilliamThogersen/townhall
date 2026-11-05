//! `/api/agents`: agents across every registered repo. Agents come only from tasks (the
//! scheduler starts them); these routes list, stop and talk to them. A name is resolved
//! across repos; `?repo=<name>` picks one when two repos have it.

use axum::extract::{Path, Query, State};
use axum::response::Response;
use serde::{Deserialize, Serialize};
use townhall_core::agent;
use townhall_core::definitions::add_tool;
use townhall_core::model::{AgentEvent, AgentKey, EntryBody};
use townhall_core::{fleet, Context, Error};

use crate::envelope::ok;
use crate::error::ApiError;
use crate::extract::JsonBody;
use crate::state::AppState;

/// `POST /api/agents/{name}/send` body: exactly one of `text` (typed as it is, then
/// Enter) and `key` (pressed alone). Unknown fields are ignored, as in phase 1.
#[derive(Debug, Deserialize)]
pub struct SendText {
    text: Option<String>,
    key: Option<AgentKey>,
}

/// What one send puts into the agent's pane.
enum Sent {
    Text(String),
    Key(AgentKey),
}

impl Sent {
    fn from_body(body: SendText) -> Result<Self, ApiError> {
        match (body.text, body.key) {
            (Some(text), None) => Ok(Self::Text(text)),
            (None, Some(key)) => Ok(Self::Key(key)),
            _ => Err(ApiError::BadRequest(
                "give exactly one of `text` and `key`".into(),
            )),
        }
    }

    /// How the send reads in a task's conversation: the text, or the key's symbol.
    fn noted(&self) -> &str {
        match self {
            Self::Text(text) => text,
            Self::Key(key) => key.symbol(),
        }
    }
}

/// `?repo=<name>` on every `/api/agents/{name}...` route.
#[derive(Debug, Default, Deserialize)]
pub struct RepoQuery {
    pub repo: Option<String>,
}

pub async fn list(State(state): State<AppState>) -> Result<Response, ApiError> {
    Ok(ok(fleet::list(state.ctx()).await?))
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
    let sent = Sent::from_body(req)?;
    let ctx = state.ctx();
    let at = fleet::locate(ctx, &name, q.repo.as_deref()).await?;
    match &sent {
        Sent::Text(text) => agent::send(&at.workspace, &name, text).await?,
        Sent::Key(key) => agent::send_key(&at.workspace, &name, *key).await?,
    }
    note_sent(ctx, &name, sent.noted()).await;
    Ok(ok(name))
}

/// A task's agent got text or a key: add `→ sent to <agent>` to its conversation. It was
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

#[derive(Debug, Deserialize)]
pub struct AllowTool {
    tool: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Allowed {
    agent_type: String,
    tool: String,
    added: bool,
}

pub async fn allow(
    State(state): State<AppState>,
    Path(name): Path<String>,
    JsonBody(req): JsonBody<AllowTool>,
) -> Result<Response, ApiError> {
    let store = state.ctx().store()?;
    let task = store
        .find_task(&name)
        .await?
        .ok_or_else(|| Error::missing("task", &name))?;
    let found = store
        .agent_type(&task.agent)
        .await?
        .ok_or_else(|| Error::missing("agent type", &task.agent))?;
    let tool = req.tool.trim().to_string();
    let edited = add_tool(&found.source, &tool).map_err(ApiError::BadRequest)?;
    let added = edited.is_some();
    if let Some(source) = edited {
        store.save_agent_type(&source).await?;
        let body = EntryBody::Agent {
            agent: task.name,
            event: AgentEvent::Allowed,
            text: format!("{tool} to {}", found.def.name),
        };
        store.append_entry(&task.conversation_id, body).await?;
    }
    Ok(ok(Allowed {
        agent_type: found.def.name,
        tool,
        added,
    }))
}
