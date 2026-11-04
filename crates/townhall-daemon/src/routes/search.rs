//! `GET /api/search?q=&limit=` (unit 3.3): conversations by title and by the text of your and
//! townhall's messages. The work happens in `Store::search`; see `docs/daemon-api.md`.

use axum::extract::rejection::QueryRejection;
use axum::extract::{Query, State};
use axum::response::Response;
use serde::Deserialize;
use townhall_core::search::SearchQuery;

use crate::envelope::ok;
use crate::error::ApiError;
use crate::state::AppState;

/// Raw query values, checked by `SearchQuery::parse` so every refusal has the same words.
#[derive(Debug, Default, Deserialize)]
pub struct SearchParams {
    q: Option<String>,
    limit: Option<String>,
}

/// `SearchHit[]`, newest activity first. `400` for a text outside 1 to 200 characters after
/// trimming, or a limit outside 1 to 200.
pub async fn search(
    State(state): State<AppState>,
    query: Result<Query<SearchParams>, QueryRejection>,
) -> Result<Response, ApiError> {
    let Query(params) = query.map_err(|e| ApiError::BadRequest(e.body_text()))?;
    let wanted = SearchQuery::parse(params.q.as_deref(), params.limit.as_deref())
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;
    Ok(ok(state.ctx().store()?.search(wanted).await?))
}
