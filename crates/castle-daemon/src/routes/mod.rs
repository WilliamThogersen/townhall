mod agents;
mod stream;

use axum::routing::{get, post};
use axum::Router;

use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    let api = Router::new()
        .route("/agents", get(agents::list).post(agents::create))
        .route("/agents/{name}", axum::routing::delete(agents::remove))
        .route("/agents/{name}/send", post(agents::send))
        .route("/agents/{name}/output", get(agents::output))
        .route("/agents/{name}/stream", get(stream::stream));
    Router::new().nest("/api", api).with_state(state)
}
