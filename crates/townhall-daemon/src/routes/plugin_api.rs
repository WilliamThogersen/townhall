use axum::extract::rejection::QueryRejection;
use axum::extract::{FromRequestParts, Path, Query, State};
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use axum::response::Response;
use serde::{Deserialize, Serialize};
use serde_json::json;
use townhall_core::model::{NotifyWhen, RepoUnclear};
use townhall_core::plugins::{storage, tokens};
use townhall_core::{fleet, Error};

use super::agents::RepoQuery;
use super::search::SearchParams;
use super::tasks::TaskQuery;
use super::{artifacts, conversations, stats, tasks};
use crate::envelope::ok;
use crate::error::ApiError;
use crate::extract::JsonBody;
use crate::state::AppState;

const API_VERSION: u32 = 1;
const NO_TOKEN: &str = "send `Authorization: Bearer <token>` with a plugin token";
const BAD_TOKEN: &str = "unknown or revoked token";

#[derive(Debug)]
pub struct Caller(tokens::Caller);

fn bearer(parts: &Parts) -> Option<&str> {
    let value = parts.headers.get(AUTHORIZATION)?.to_str().ok()?;
    let (scheme, token) = value.split_once(' ')?;
    let token = token.trim();
    (scheme.eq_ignore_ascii_case("bearer") && !token.is_empty()).then_some(token)
}

impl FromRequestParts<AppState> for Caller {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = bearer(parts).ok_or_else(|| ApiError::Unauthorized(NO_TOKEN.into()))?;
        let caller = tokens::authenticate(state.ctx().store()?, token).await?;
        caller
            .map(Self)
            .ok_or_else(|| ApiError::Unauthorized(BAD_TOKEN.into()))
    }
}

impl Caller {
    fn require(&self, scope: &str) -> Result<(), ApiError> {
        if self.0.allows(scope) {
            Ok(())
        } else {
            Err(ApiError::Forbidden(format!(
                "this token has no `{scope}` scope"
            )))
        }
    }

    fn own_task(&self, name: &str) -> Result<(), ApiError> {
        match &self.0.task {
            Some(own) if !own.eq_ignore_ascii_case(name) => Err(ApiError::Forbidden(
                "a task token only sees its own task".into(),
            )),
            _ => Ok(()),
        }
    }

    fn not_for_task_tokens(&self, what: &str) -> Result<(), ApiError> {
        match self.0.task {
            Some(_) => Err(ApiError::Forbidden(format!(
                "a task token cannot read {what}"
            ))),
            None => Ok(()),
        }
    }

    async fn own_conversation(&self, state: &AppState) -> Result<Option<String>, ApiError> {
        let Some(task) = &self.0.task else {
            return Ok(None);
        };
        match state.ctx().store()?.find_task(task).await? {
            Some(found) => Ok(Some(found.conversation_id)),
            None => Err(Error::missing("task", task).into()),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Info {
    api_version: u32,
    townhall: &'static str,
    plugin: String,
    scopes: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TownhallSettings {
    agents_at_once: u32,
    repo_unclear: RepoUnclear,
    session_panel_open: bool,
    notify_when: NotifyWhen,
    min_free_disk_gb: u32,
    learn_about_me: bool,
    reflect_after_tasks: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PutValue {
    value: String,
}

pub async fn info(caller: Caller) -> Response {
    ok(Info {
        api_version: API_VERSION,
        townhall: townhall_core::version::VERSION,
        plugin: caller.0.plugin,
        scopes: caller.0.scopes,
    })
}

pub async fn task_list(
    caller: Caller,
    State(state): State<AppState>,
    query: Result<Query<TaskQuery>, QueryRejection>,
) -> Result<Response, ApiError> {
    caller.require("tasks:read")?;
    let mut found = tasks::select(&state, query).await?;
    if let Some(own) = &caller.0.task {
        found.retain(|t| t.name.eq_ignore_ascii_case(own));
    }
    Ok(ok(found))
}

pub async fn task_get(
    caller: Caller,
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Response, ApiError> {
    caller.require("tasks:read")?;
    caller.own_task(&name)?;
    tasks::get(State(state), Path(name)).await
}

pub async fn task_artifacts(
    caller: Caller,
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Response, ApiError> {
    caller.require("tasks:read")?;
    caller.own_task(&name)?;
    artifacts::list(State(state), Path(name)).await
}

pub async fn agent_list(
    caller: Caller,
    State(state): State<AppState>,
) -> Result<Response, ApiError> {
    caller.require("tasks:read")?;
    let mut found = fleet::list(state.ctx()).await?;
    if let Some(own) = &caller.0.task {
        found.retain(|a| a.name.eq_ignore_ascii_case(own));
    }
    Ok(ok(found))
}

pub async fn stats(
    caller: Caller,
    State(state): State<AppState>,
    Query(query): Query<RepoQuery>,
) -> Result<Response, ApiError> {
    caller.require("tasks:read")?;
    caller.not_for_task_tokens("the stats of every repo")?;
    stats::get(State(state), Query(query)).await
}

pub async fn conversation_list(
    caller: Caller,
    State(state): State<AppState>,
) -> Result<Response, ApiError> {
    caller.require("conversations:read")?;
    let own = caller.own_conversation(&state).await?;
    let mut found = state.ctx().store()?.conversations().await?;
    if let Some(own) = own {
        found.retain(|c| c.id == own);
    }
    Ok(ok(found))
}

pub async fn conversation_detail(
    caller: Caller,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    caller.require("conversations:read")?;
    if let Some(own) = caller.own_conversation(&state).await? {
        if own != id {
            return Err(ApiError::Forbidden(
                "a task token only sees the conversation of its task".into(),
            ));
        }
    }
    conversations::detail(State(state), Path(id)).await
}

pub async fn search(
    caller: Caller,
    State(state): State<AppState>,
    query: Result<Query<SearchParams>, QueryRejection>,
) -> Result<Response, ApiError> {
    caller.require("conversations:read")?;
    caller.not_for_task_tokens("a search across every conversation")?;
    super::search::search(State(state), query).await
}

pub async fn settings(caller: Caller, State(state): State<AppState>) -> Result<Response, ApiError> {
    caller.require("settings:read")?;
    let all = state.ctx().settings().await?;
    Ok(ok(TownhallSettings {
        agents_at_once: all.agents_at_once,
        repo_unclear: all.repo_unclear,
        session_panel_open: all.session_panel_open,
        notify_when: all.notify_when,
        min_free_disk_gb: all.min_free_disk_gb,
        learn_about_me: all.learn_about_me,
        reflect_after_tasks: all.reflect_after_tasks,
    }))
}

pub async fn storage_get(
    caller: Caller,
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<Response, ApiError> {
    caller.require("storage")?;
    let value = storage::get(state.ctx().store()?, &caller.0.plugin, &key).await?;
    Ok(ok(json!({ "key": key, "value": value })))
}

pub async fn storage_put(
    caller: Caller,
    State(state): State<AppState>,
    Path(key): Path<String>,
    JsonBody(body): JsonBody<PutValue>,
) -> Result<Response, ApiError> {
    caller.require("storage")?;
    storage::put(state.ctx().store()?, &caller.0.plugin, &key, &body.value).await?;
    Ok(ok(json!({ "key": key })))
}

pub async fn storage_delete(
    caller: Caller,
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<Response, ApiError> {
    caller.require("storage")?;
    storage::delete(state.ctx().store()?, &caller.0.plugin, &key).await?;
    Ok(ok(json!({ "key": key })))
}
