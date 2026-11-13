use axum::extract::State;
use axum::response::Response;
use serde::{Deserialize, Serialize};
use serde_json::json;
use townhall_core::definitions::add_tool;
use townhall_core::Error;

use crate::envelope::ok;
use crate::error::ApiError;
use crate::extract::JsonBody;
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Accept {
    agent_type: String,
    tool: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Accepted {
    agent_type: String,
    tool: String,
    added: bool,
}

pub async fn accept(
    State(state): State<AppState>,
    JsonBody(req): JsonBody<Accept>,
) -> Result<Response, ApiError> {
    let store = state.ctx().store()?;
    let found = store
        .agent_type(&req.agent_type)
        .await?
        .ok_or_else(|| Error::missing("agent type", &req.agent_type))?;
    let tool = req.tool.trim().to_string();
    let edited = add_tool(&found.source, &tool).map_err(ApiError::BadRequest)?;
    let added = edited.is_some();
    if let Some(source) = edited {
        store.save_agent_type(&source).await?;
    }
    Ok(ok(Accepted {
        agent_type: found.def.name,
        tool,
        added,
    }))
}
