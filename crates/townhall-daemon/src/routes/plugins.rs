use axum::extract::{Path, Query, State};
use axum::response::Response;
use serde::Deserialize;
use serde_json::Value;
use townhall_core::plugins::declared;
use townhall_core::plugins::install::{InstallPlugin, PatchPlugin, PluginRegistry};

use crate::envelope::{created, ok};
use crate::error::ApiError;
use crate::extract::JsonBody;
use crate::state::AppState;

fn registry(state: &AppState) -> Result<PluginRegistry, ApiError> {
    Ok(PluginRegistry::new(state.ctx().store()?.clone()))
}

#[derive(Debug, Deserialize)]
pub struct PreviewQuery {
    plugin: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct UpdateQuery {
    #[serde(default)]
    approve: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct UninstallQuery {
    #[serde(default)]
    purge: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeBody {
    #[serde(deserialize_with = "Option::deserialize")]
    plugin: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingBody {
    value: Value,
}

pub async fn contributions(State(state): State<AppState>) -> Result<Response, ApiError> {
    Ok(ok(declared::contributions(state.ctx().store()?).await?))
}

pub async fn put_theme(
    State(state): State<AppState>,
    JsonBody(req): JsonBody<ThemeBody>,
) -> Result<Response, ApiError> {
    Ok(ok(declared::choose_theme(
        state.ctx().store()?,
        req.plugin.as_deref(),
    )
    .await?))
}

pub async fn put_setting(
    State(state): State<AppState>,
    Path((id, key)): Path<(String, String)>,
    JsonBody(req): JsonBody<SettingBody>,
) -> Result<Response, ApiError> {
    Ok(ok(declared::set_setting(
        state.ctx().store()?,
        &id,
        &key,
        req.value,
    )
    .await?))
}

pub async fn reset_setting(
    State(state): State<AppState>,
    Path((id, key)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    Ok(ok(
        declared::reset_setting(state.ctx().store()?, &id, &key).await?
    ))
}

pub async fn list(State(state): State<AppState>) -> Result<Response, ApiError> {
    Ok(ok(registry(&state)?.list().await?))
}

pub async fn preview(
    State(state): State<AppState>,
    Query(query): Query<PreviewQuery>,
) -> Result<Response, ApiError> {
    let plugin = query
        .plugin
        .ok_or_else(|| ApiError::BadRequest("plugin is required: name@marketplace".into()))?;
    Ok(ok(registry(&state)?.preview(&plugin).await?))
}

pub async fn install(
    State(state): State<AppState>,
    JsonBody(req): JsonBody<InstallPlugin>,
) -> Result<Response, ApiError> {
    Ok(created(registry(&state)?.install(&req).await?))
}

pub async fn patch(
    State(state): State<AppState>,
    Path(id): Path<String>,
    JsonBody(req): JsonBody<PatchPlugin>,
) -> Result<Response, ApiError> {
    Ok(ok(registry(&state)?.patch(&id, &req).await?))
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<UpdateQuery>,
) -> Result<Response, ApiError> {
    Ok(ok(registry(&state)?
        .update(&id, query.approve.as_deref())
        .await?))
}

pub async fn uninstall(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<UninstallQuery>,
) -> Result<Response, ApiError> {
    Ok(ok(registry(&state)?.uninstall(&id, query.purge).await?))
}
