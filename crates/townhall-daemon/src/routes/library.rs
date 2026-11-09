//! The definitions library (unit 2.8b, agent types from 2.12): agent types saved as TOML,
//! and their import. Bodies are the TOML text (`text/plain` or `application/toml`); a type
//! that fails 2.8a's checks is `400` with every `DefinitionError` as `data`. Workflows wait
//! for 2.8c and still answer `501`.

use axum::body::Bytes;
use std::time::SystemTime;

use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use townhall_core::agent_usage::UsageRange;
use townhall_core::clock;
use townhall_core::definitions::{AgentDef, DefKind, DefinitionError, Parsed};
use townhall_core::model::{AgentType, AgentTypeSummary};
use townhall_core::store::Store;
use townhall_core::Error;

use super::events::FOREIGN_ORIGIN;
use crate::cors::origin_allowed;
use crate::envelope::{created, failure, ok, Envelope};
use crate::error::ApiError;
use crate::state::AppState;

const TOML_TYPES: [&str; 2] = ["text/plain", "application/toml"];
const WRONG_TYPE: &str = "expected content-type text/plain or application/toml";

/// `400` with the error list as `data` and every error on one line as `error`.
fn invalid(errors: Vec<DefinitionError>) -> Response {
    let text = errors
        .iter()
        .map(|e| {
            let line = e.to_string();
            line.strip_prefix("error: ").unwrap_or(&line).to_string()
        })
        .collect::<Vec<_>>()
        .join("; ");
    let body = Envelope {
        success: false,
        data: Some(errors),
        error: Some(text),
    };
    (StatusCode::BAD_REQUEST, Json(body)).into_response()
}

/// The body as text, when it was sent as TOML text.
fn toml_text(headers: &HeaderMap, body: &Bytes) -> Result<String, ApiError> {
    let kind = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .map(|v| v.trim().to_ascii_lowercase());
    if !kind.is_some_and(|k| TOML_TYPES.contains(&k.as_str())) {
        return Err(ApiError::BadRequest(WRONG_TYPE.into()));
    }
    String::from_utf8(body.to_vec())
        .map_err(|_| ApiError::BadRequest("the toml must be utf-8 text".into()))
}

fn name_mismatch(def: &AgentDef, path: &str) -> DefinitionError {
    DefinitionError {
        kind: DefKind::Agent,
        name: Some(def.name.clone()),
        step: None,
        field: Some("name".into()),
        message: format!("must match the path: \"{path}\""),
        line: None,
    }
}

/// A `[[step]]` table: the shape of a workflow.
fn looks_like_workflow(source: &str) -> bool {
    source.lines().any(|line| {
        let tight: String = line.chars().filter(|c| !c.is_whitespace()).collect();
        tight == "[[step]]"
    })
}

async fn save(state: &AppState, parsed: Parsed<AgentDef>) -> Result<Response, ApiError> {
    let store = state.ctx().store()?;
    let (saved, replaced) = store.save_agent_type(&parsed.source).await?;
    Ok(if replaced { ok(saved) } else { created(saved) })
}

/// `AgentTypeSummary[]` by name, ignoring case.
pub async fn list_agents(State(state): State<AppState>) -> Result<Response, ApiError> {
    let types = state.ctx().store()?.agent_types().await?;
    let rows: Vec<AgentTypeSummary> = types.iter().map(AgentType::summary).collect();
    Ok(ok(rows))
}

pub async fn agent_usage(State(state): State<AppState>) -> Result<Response, ApiError> {
    Ok(ok(state.ctx().store()?.agent_usage().await?))
}

#[derive(serde::Deserialize)]
pub struct UsageQuery {
    range: Option<String>,
}

/// `AgentUsageSeries` of one type: `?range=24h|7d|30d`, `24h` when left out.
pub async fn agent_usage_series(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(q): Query<UsageQuery>,
) -> Result<Response, ApiError> {
    let label = q.range.as_deref().unwrap_or("24h");
    let range = UsageRange::parse(label).ok_or_else(|| {
        ApiError::BadRequest(format!("range \"{label}\" is not one of 24h, 7d, 30d"))
    })?;
    let now_ms = clock::unix_millis(SystemTime::now());
    let series = state
        .ctx()
        .store()?
        .agent_usage_series(&name, range, now_ms)
        .await?;
    Ok(ok(series))
}

/// `AgentType`: the parsed definition plus `source` and `updatedAt`. `404` when unknown.
pub async fn get_agent(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Response, ApiError> {
    match state.ctx().store()?.agent_type(&name).await? {
        Some(found) => Ok(ok(found)),
        None => Err(Error::missing("agent type", name).into()),
    }
}

/// Saves the TOML body: `201` new, `200` replaced. Its `name` must be the path's.
pub async fn put_agent(
    State(state): State<AppState>,
    Path(name): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ApiError> {
    let source = toml_text(&headers, &body)?;
    let parsed = match Store::check_agent_type(&source) {
        Ok(parsed) => parsed,
        Err(errors) => return Ok(invalid(errors)),
    };
    if parsed.def.name != name {
        return Ok(invalid(vec![name_mismatch(&parsed.def, &name)]));
    }
    save(&state, parsed).await
}

/// The deleted type's name. `404` unknown, `409` the last type.
pub async fn delete_agent(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Response, ApiError> {
    Ok(ok(state.ctx().store()?.delete_agent_type(&name).await?))
}

pub async fn list_workflows() -> ApiError {
    ApiError::NotImplemented
}

pub async fn get_workflow() -> ApiError {
    ApiError::NotImplemented
}

pub async fn put_workflow() -> ApiError {
    ApiError::NotImplemented
}

pub async fn delete_workflow() -> ApiError {
    ApiError::NotImplemented
}

/// One `.toml` agent type as the body: `201` with the saved type. `403` foreign origin
/// (a `text/plain` post needs no CORS preflight), `409` the name is taken, `501` a workflow.
pub async fn import(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ApiError> {
    if !origin_allowed(&headers) {
        return Ok(failure(StatusCode::FORBIDDEN, FOREIGN_ORIGIN.into()));
    }
    let source = toml_text(&headers, &body)?;
    if looks_like_workflow(&source) {
        return Err(ApiError::NotImplemented);
    }
    let parsed = match Store::check_agent_type(&source) {
        Ok(parsed) => parsed,
        Err(errors) => return Ok(invalid(errors)),
    };
    let store = state.ctx().store()?;
    if let Some(taken) = store.agent_type(&parsed.def.name).await? {
        let name = taken.def.name;
        let why =
            format!("agent type `{name}` exists: replace it with PUT /api/library/agents/{name}");
        return Err(Error::Conflict(why).into());
    }
    save(&state, parsed).await
}
