//! `read_logs` and `log_summary`: townhall's planner reads its own logs
//! (`<home>/logs/*.jsonl`, docs/logging.md). The work is `crate::logs`; this file has the
//! tool schemas and runs the reader on a blocking thread.

use std::time::SystemTime;

use serde_json::{json, Value};

use crate::clock::unix_millis;
use crate::context::Context;
use crate::logs::{self, ReadArgs, SummaryArgs};

use super::checks::Checked;
use super::ToolDef;

pub const READ_LOGS: &str = "read_logs";
pub const LOG_SUMMARY: &str = "log_summary";

const LEVEL_DESCRIPTION: &str = "Minimum level: error, warn, info, debug or trace.";
const SINCE_DESCRIPTION: &str =
    "How far back: a duration like 24h, 90m or 7d, or an RFC 3339 time. Default 24h.";

pub fn tools() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: READ_LOGS,
            description: "Read townhall's own log lines (daemon and desktop app), newest first, as `ts level target event message {ids}`. Default: warnings and errors of the last 24h. Filter by level, component (target prefix such as `git` or `townhall::runner`), event, task or a substring. Output is capped; `truncated: true` says more matched.",
            input_schema: read_schema(),
            read_only: true,
        },
        ToolDef {
            name: LOG_SUMMARY,
            description: "A digest of townhall's own logs: counts per level and component, every panic and the top recurring groups (same target, event and message once numbers, ids, paths and names are ignored) with count, first and last seen, up to 5 tasks touched, one sample and a stable `fingerprint` to cite in a task brief. Start here when asked what is going wrong in townhall or to improve it.",
            input_schema: summary_schema(),
            read_only: true,
        },
    ]
}

fn read_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "level": { "type": "string", "enum": ["error", "warn", "info", "debug", "trace"], "description": format!("{LEVEL_DESCRIPTION} Default warn.") },
            "component": { "type": "string", "description": "Target prefix, e.g. `http`, `git`, `task`, `runner`, `agent`, `mcp`, `web` or the full `townhall::git`." },
            "event": { "type": "string", "description": "Exact event name, e.g. `task.launch_failed`." },
            "task": { "type": "string", "description": "Only lines about this task." },
            "grep": { "type": "string", "description": "Case-insensitive substring of the message or error." },
            "since": { "type": "string", "description": SINCE_DESCRIPTION },
            "limit": { "type": "integer", "minimum": 1, "maximum": logs::MAX_LIMIT, "description": "Lines to return, default 50." }
        },
        "additionalProperties": false
    })
}

fn summary_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "since": { "type": "string", "description": SINCE_DESCRIPTION },
            "level": { "type": "string", "enum": ["error", "warn", "info", "debug", "trace"], "description": format!("{LEVEL_DESCRIPTION} Default warn.") },
            "top": { "type": "integer", "minimum": 1, "maximum": logs::MAX_TOP, "description": "Recurring groups to return, default 20." }
        },
        "additionalProperties": false
    })
}

fn now_ms() -> i64 {
    unix_millis(SystemTime::now())
}

pub(super) async fn read(ctx: &Context, args: ReadArgs) -> Checked<Value> {
    let dir = ctx.home().logs_dir().map_err(|e| e.to_string())?;
    let out = tokio::task::spawn_blocking(move || logs::read_logs(&dir, now_ms(), &args))
        .await
        .map_err(|e| format!("reading logs failed: {e}"))??;
    serde_json::to_value(out).map_err(|e| e.to_string())
}

pub(super) async fn summary(ctx: &Context, args: SummaryArgs) -> Checked<Value> {
    let dir = ctx.home().logs_dir().map_err(|e| e.to_string())?;
    let out = tokio::task::spawn_blocking(move || logs::log_summary(&dir, now_ms(), &args))
        .await
        .map_err(|e| format!("summarizing logs failed: {e}"))??;
    serde_json::to_value(out).map_err(|e| e.to_string())
}
