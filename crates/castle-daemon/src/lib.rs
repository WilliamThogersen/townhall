//! castle-daemon: HTTP and WebSocket API over castle-core agents.

pub mod config;
pub mod envelope;
pub mod error;
pub mod routes;
pub mod state;

pub use routes::router;
pub use state::AppState;
