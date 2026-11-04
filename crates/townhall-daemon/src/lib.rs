//! townhall-daemon: HTTP and WebSocket API over townhall-core agents.

pub mod config;
pub mod cors;
pub mod envelope;
pub mod error;
pub mod routes;
pub mod serve;
pub mod state;

pub use routes::router;
pub use serve::serve;
pub use state::AppState;

/// The API router wrapped with the desktop/dev CORS policy.
pub fn router_with_cors(state: AppState) -> axum::Router {
    router(state).layer(cors::cors_layer())
}
