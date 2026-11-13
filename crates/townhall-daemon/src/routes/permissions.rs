use axum::extract::State;
use axum::response::Response;
use serde_json::json;

use crate::envelope::ok;
use crate::error::ApiError;
use crate::state::AppState;

const DECISIONS_SHOWN: u32 = 50;

pub async fn get(State(state): State<AppState>) -> Result<Response, ApiError> {
    let store = state.ctx().store()?;
    let decisions = store.permission_decisions(DECISIONS_SHOWN).await?;
    let suggestions = store.permission_suggestions().await?;
    Ok(ok(
        json!({"decisions": decisions, "suggestions": suggestions}),
    ))
}
