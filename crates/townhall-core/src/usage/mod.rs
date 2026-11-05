//! Claude plan usage (the numbers behind Claude Code's `/usage`), read with Claude Code's own
//! OAuth token and served by `GET /api/usage`.

mod parse;
mod rate;

use serde::{Deserialize, Serialize};

pub use parse::{parse_usage, token_from_json, usage_error};
pub use rate::{
    push_reading, recent_rate, session_rate, weekly_rate, window_average_rate, Reading,
};

/// One rate-limit window (the five hour session or the week).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageWindow {
    /// 0 to 100.
    pub used_percent: f64,
    /// Unix milliseconds.
    pub resets_at: Option<i64>,
    /// Percentage points per hour; `None` when unknown.
    pub rate_per_hour: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeUsage {
    pub session: Option<UsageWindow>,
    pub weekly: Option<UsageWindow>,
    /// Unix milliseconds of the last successful read.
    pub updated_at: i64,
    /// Why the numbers could not be refreshed; the windows then hold the last good reading.
    pub error: Option<String>,
}
