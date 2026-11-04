//! The MCP endpoint (unit 2.3): JSON-RPC 2.0 for townhall's session, answered by
//! `townhall_core::mcp::handle`. Its answers are JSON-RPC, not the envelope; an unknown
//! conversation or a body that is not JSON is the usual envelope error.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::Value;
use townhall_core::mcp;

use crate::error::ApiError;
use crate::extract::JsonBody;
use crate::state::AppState;

/// `POST /api/conversations/{id}/mcp`: the answer, or `202` with no body for notifications.
pub async fn rpc(
    State(state): State<AppState>,
    Path(id): Path<String>,
    JsonBody(body): JsonBody<Value>,
) -> Result<Response, ApiError> {
    let ctx = state.ctx();
    ctx.store()?.conversation(&id).await?;
    Ok(match mcp::handle(ctx, &id, body).await {
        Some(answer) => Json(answer).into_response(),
        None => StatusCode::ACCEPTED.into_response(),
    })
}
