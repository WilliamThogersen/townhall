//! `/api/settings`: the `settings` table. `PUT /api/settings/{key}` is the only way to change
//! a setting. Permissions are not settings: each agent type sets its own.

use axum::extract::{Path, State};
use axum::response::Response;
use townhall_core::model::{PutSetting, SettingKey};

use crate::envelope::ok;
use crate::error::ApiError;
use crate::extract::JsonBody;
use crate::state::AppState;

pub async fn get(State(state): State<AppState>) -> Result<Response, ApiError> {
    Ok(ok(state.ctx().settings().await?))
}

/// Body `{"value": <json>}`; answers with every setting after the change.
pub async fn put(
    State(state): State<AppState>,
    Path(key): Path<String>,
    JsonBody(body): JsonBody<PutSetting>,
) -> Result<Response, ApiError> {
    let key = SettingKey::parse(&key)?;
    let store = state.ctx().store()?;
    Ok(ok(store.set_setting(key, body.value).await?))
}
