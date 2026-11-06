//! `/api/profile`: the one text the user keeps about how they work. It lives in the local
//! store and goes nowhere but the planner's system prompt, and only while `learnAboutMe` is on.

use axum::extract::State;
use axum::response::Response;
use serde::Serialize;
use townhall_core::model::PutProfile;

use crate::envelope::ok;
use crate::error::ApiError;
use crate::extract::JsonBody;
use crate::state::AppState;

#[derive(Serialize)]
struct Profile {
    text: String,
}

/// `{ text }`, empty when none was saved.
pub async fn get(State(state): State<AppState>) -> Result<Response, ApiError> {
    let text = state.ctx().store()?.profile().await?;
    Ok(ok(Profile { text }))
}

/// Body `{ text }` replaces the profile; `400` past `MAX_PROFILE_CHARS`.
pub async fn put(
    State(state): State<AppState>,
    JsonBody(body): JsonBody<PutProfile>,
) -> Result<Response, ApiError> {
    let text = state.ctx().store()?.set_profile(&body.text).await?;
    Ok(ok(Profile { text }))
}

/// Resets the profile to empty and answers `{ text: "" }`.
pub async fn remove(State(state): State<AppState>) -> Result<Response, ApiError> {
    state.ctx().store()?.delete_profile().await?;
    Ok(ok(Profile {
        text: String::new(),
    }))
}
