use axum::extract::{Path, State};
use axum::response::Response;
use townhall_core::plugins::market::{AddMarketplace, MarketplaceRegistry};

use crate::envelope::{created, ok};
use crate::error::ApiError;
use crate::extract::JsonBody;
use crate::state::AppState;

fn registry(state: &AppState) -> Result<MarketplaceRegistry, ApiError> {
    Ok(MarketplaceRegistry::new(state.ctx().store()?.clone()))
}

pub async fn list(State(state): State<AppState>) -> Result<Response, ApiError> {
    Ok(ok(registry(&state)?.list().await?))
}

pub async fn add(
    State(state): State<AppState>,
    JsonBody(req): JsonBody<AddMarketplace>,
) -> Result<Response, ApiError> {
    Ok(created(registry(&state)?.add(&req).await?))
}

pub async fn remove(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Response, ApiError> {
    Ok(ok(registry(&state)?.remove(&name).await?))
}

pub async fn refresh(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Response, ApiError> {
    Ok(ok(registry(&state)?.refresh(&name).await?))
}
