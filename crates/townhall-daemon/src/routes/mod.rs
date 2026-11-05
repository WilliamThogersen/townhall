//! Every phase 2 route, registered once here. Each unit fills in the handlers of its own
//! file and does not edit this one (ownership map: `docs/daemon-api.md`).

mod agents;
mod artifacts;
mod attach;
mod attach_frames;
mod attachments;
mod conversations;
mod events;
mod info;
mod library;
mod logs;
mod mcp;
mod repos;
mod runs;
mod search;
mod settings;
mod stats;
mod steps;
mod stream;
mod tasks;
mod usage;

use axum::extract::DefaultBodyLimit;
use axum::routing::{delete, get, post, put};
use axum::Router;
use townhall_core::model::{MAX_ARTIFACT_BYTES, MAX_ATTACHMENT_BYTES};

use crate::state::AppState;

/// One attachment per upload request, plus room for the multipart framing.
pub const UPLOAD_BODY_LIMIT: usize = MAX_ATTACHMENT_BYTES as usize + 64 * 1024;

pub const START_BODY_LIMIT: usize = 5 * MAX_ATTACHMENT_BYTES as usize + 64 * 1024;

/// One artifact as base64 inside a JSON-RPC call, plus room for the rest of the call.
pub const TASK_MCP_BODY_LIMIT: usize = MAX_ARTIFACT_BYTES as usize / 3 * 4 + 64 * 1024;

fn agent_routes() -> Router<AppState> {
    Router::new()
        .route("/agents", get(agents::list))
        .route("/agents/{name}", delete(agents::remove))
        .route("/agents/{name}/send", post(agents::send))
        .route("/agents/{name}/allow", post(agents::allow))
        .route("/agents/{name}/output", get(agents::output))
        .route("/agents/{name}/stream", get(stream::stream))
        .route("/agents/{name}/attach", get(attach::attach))
        .route("/agents/{name}/steps", get(steps::list))
}

fn conversation_routes() -> Router<AppState> {
    let upload = get(attachments::list)
        .post(attachments::upload)
        .layer(DefaultBodyLimit::max(UPLOAD_BODY_LIMIT));
    let start = get(conversations::list)
        .post(conversations::create)
        .layer(DefaultBodyLimit::max(START_BODY_LIMIT));
    Router::new()
        .route("/conversations", start)
        .route(
            "/conversations/{id}",
            get(conversations::detail).patch(conversations::patch),
        )
        .route("/conversations/{id}/messages", post(conversations::send))
        .route("/conversations/{id}/stop", post(conversations::stop))
        .route("/conversations/{id}/end", post(conversations::end))
        .route("/conversations/{id}/mcp", post(mcp::rpc))
        .route("/conversations/{id}/attachments", upload)
        .route(
            "/conversations/{id}/attachments/{attachment}",
            get(attachments::download),
        )
}

fn library_routes() -> Router<AppState> {
    Router::new()
        .route("/library/agents", get(library::list_agents))
        .route("/library/agents/usage", get(library::agent_usage))
        .route(
            "/library/agents/{name}",
            get(library::get_agent)
                .put(library::put_agent)
                .delete(library::delete_agent),
        )
        .route("/library/workflows", get(library::list_workflows))
        .route(
            "/library/workflows/{name}",
            get(library::get_workflow)
                .put(library::put_workflow)
                .delete(library::delete_workflow),
        )
        .route("/library/import", post(library::import))
        .route("/workflows/{name}/runs", post(runs::start))
        .route("/runs/{id}", get(runs::get))
}

pub fn router(state: AppState) -> Router {
    let api = Router::new()
        .route("/info", get(info::info))
        .route("/repos", get(repos::list).post(repos::add))
        .route("/repos/{name}", delete(repos::remove))
        .route("/settings", get(settings::get))
        .route("/settings/{key}", put(settings::put))
        .route("/tasks", get(tasks::list))
        .route("/tasks/{name}", get(tasks::get))
        .route(
            "/tasks/{name}/mcp",
            post(mcp::task_rpc).layer(DefaultBodyLimit::max(TASK_MCP_BODY_LIMIT)),
        )
        .route("/tasks/{name}/artifacts", get(artifacts::list))
        .route("/tasks/{name}/artifacts/{id}", get(artifacts::download))
        .route("/stats", get(stats::get))
        .route("/usage", get(usage::get))
        .route("/search", get(search::search))
        .route("/events", get(events::socket))
        .merge(agent_routes())
        .merge(conversation_routes())
        .merge(library_routes())
        .merge(logs::routes());
    Router::new().nest("/api", api).with_state(state)
}
