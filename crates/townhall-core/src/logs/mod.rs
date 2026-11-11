//! Reads townhall's own JSONL logs (`<home>/logs/*.jsonl`, written by `townhall-daemon`'s
//! `logging`, format in docs/logging.md) for the planner's `read_logs` and `log_summary` tools
//! and the `GET /api/logs` routes.
//!
//! Pure and synchronous: callers on the async runtime go through `spawn_blocking`. The output
//! caps (300 chars a line, 16 KB in all) are enforced here, not by the callers.

mod fingerprint;
mod time;

use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::redact::redact;

pub use fingerprint::{fingerprint, normalize};
pub use time::{parse_rfc3339, parse_since};

/// Everything a tool or route returns stays under this many bytes of JSON.
pub const MAX_OUTPUT_BYTES: usize = 16 * 1024;
/// A `read_logs` line is cut to this many chars.
pub const MAX_LINE_CHARS: usize = 300;
pub const DEFAULT_LIMIT: usize = 50;
pub const MAX_LIMIT: usize = 200;
pub const DEFAULT_TOP: usize = 20;
pub const MAX_TOP: usize = 50;
pub const DEFAULT_SINCE: &str = "24h";
pub const DEFAULT_LEVEL: &str = "warn";
const MAX_SAMPLE_CHARS: usize = 200;
const MAX_TASKS_PER_GROUP: usize = 5;
const MS_PER_DAY: i64 = 86_400_000;
/// Fields shown in braces after the message, in this order.
const SHOWN_FIELDS: [&str; 11] = [
    "task",
    "agent",
    "conversation",
    "repo",
    "request_id",
    "tool",
    "status",
    "duration_ms",
    "from",
    "to",
    "error_kind",
];
/// Fields whose values are placeholders in a fingerprint.
const ID_FIELDS: [&str; 5] = ["task", "agent", "conversation", "repo", "request_id"];

/// `read_logs` arguments (the MCP tool's and the `GET /api/logs` query's).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadArgs {
    /// Minimum level; default `warn`.
    pub level: Option<String>,
    /// Target prefix, with or without the `townhall::`.
    pub component: Option<String>,
    pub event: Option<String>,
    pub task: Option<String>,
    /// Case-insensitive substring of the message or error.
    pub grep: Option<String>,
    /// `24h`, `7d` or an RFC 3339 time; default `24h`.
    pub since: Option<String>,
    /// Default 50, at most 200.
    pub limit: Option<usize>,
}

/// `log_summary` arguments.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SummaryArgs {
    pub since: Option<String>,
    pub level: Option<String>,
    /// Groups to return; default 20, at most 50.
    pub top: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReadOutput {
    /// `ts level target event message {ids}`, newest first.
    pub lines: Vec<String>,
    pub count: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ComponentCount {
    pub component: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct PanicEntry {
    pub ts: String,
    pub message: String,
    pub location: Option<String>,
    pub source: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct Group {
    pub fingerprint: String,
    pub count: usize,
    pub target: String,
    pub event: String,
    pub first_seen: String,
    pub last_seen: String,
    /// Up to 5 tasks the group touched, newest first.
    pub tasks: Vec<String>,
    /// One redacted message of the group (the newest).
    pub sample: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    pub since: String,
    pub level: String,
    pub total: usize,
    pub by_level: BTreeMap<String, usize>,
    pub by_component: Vec<ComponentCount>,
    pub panics: Vec<PanicEntry>,
    pub groups: Vec<Group>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Level {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl Level {
    fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "trace" => Some(Self::Trace),
            "debug" => Some(Self::Debug),
            "info" => Some(Self::Info),
            "warn" | "warning" => Some(Self::Warn),
            "error" => Some(Self::Error),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Trace => "TRACE",
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
        }
    }
}

fn level_arg(level: Option<&str>) -> Result<Level, String> {
    let s = level.unwrap_or(DEFAULT_LEVEL);
    Level::parse(s)
        .ok_or_else(|| format!("unknown level `{s}`: use error, warn, info, debug or trace"))
}

/// One parsed log line.
struct Entry {
    ts: String,
    ts_ms: i64,
    level: Level,
    target: String,
    event: String,
    message: String,
    source: &'static str,
    fields: Map<String, Value>,
}

impl Entry {
    fn field(&self, name: &str) -> Option<&str> {
        self.fields.get(name).and_then(Value::as_str)
    }
}

fn parse_line(line: &str, source: &'static str) -> Option<Entry> {
    let Value::Object(fields) = serde_json::from_str(line).ok()? else {
        return None;
    };
    let ts = fields.get("timestamp")?.as_str()?.to_string();
    let ts_ms = parse_rfc3339(&ts)?;
    let level = Level::parse(fields.get("level")?.as_str()?)?;
    let text = |name: &str| {
        fields
            .get(name)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    Some(Entry {
        ts_ms,
        level,
        target: text("target"),
        event: text("event"),
        message: text("message"),
        source,
        ts,
        fields,
    })
}

/// `(date, source, path)` of every `app.DATE.jsonl` and `daemon.DATE.jsonl` in `dir`.
fn log_files(dir: &Path) -> Vec<(String, &'static str, std::path::PathBuf)> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    read.filter_map(Result::ok)
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let stem = name.strip_suffix(".jsonl")?;
            let (prefix, date) = stem.split_once('.')?;
            let source = match prefix {
                "app" => "app",
                "daemon" => "daemon",
                _ => return None,
            };
            Some((date.to_string(), source, e.path()))
        })
        .collect()
}

/// Every entry at or after `since_ms` that `keep` accepts, newest first. Files dated more than
/// a day before `since` are not opened (a file's date is its rotation day, so one day of slack).
fn read_entries(dir: &Path, since_ms: i64, keep: impl Fn(&Entry) -> bool) -> Vec<Entry> {
    let oldest_day = crate::clock::from_unix_millis(since_ms.saturating_sub(MS_PER_DAY));
    let oldest_day = &oldest_day[..10];
    let mut files = log_files(dir);
    files.sort_by(|a, b| b.0.cmp(&a.0));
    let mut entries = Vec::new();
    for (date, source, path) in files {
        if date.as_str() < oldest_day {
            break;
        }
        let Ok(file) = File::open(&path) else {
            continue;
        };
        for line in BufReader::new(file).split(b'\n').map_while(Result::ok) {
            let Ok(line) = String::from_utf8(line) else {
                continue;
            };
            if let Some(entry) = parse_line(&line, source) {
                if entry.ts_ms >= since_ms && keep(&entry) {
                    entries.push(entry);
                }
            }
        }
    }
    entries.sort_by_key(|a| std::cmp::Reverse(a.ts_ms));
    entries
}

fn one_line(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

fn cut_chars(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max - 1).collect();
    out.push('…');
    out
}

/// `ts level target event message {source=.. task=..}`, at most 300 chars.
fn format_line(e: &Entry) -> String {
    let mut ids = vec![format!("source={}", e.source)];
    for name in SHOWN_FIELDS {
        match e.fields.get(name) {
            Some(Value::String(s)) => ids.push(format!("{name}={}", one_line(&redact(s)))),
            Some(v @ (Value::Number(_) | Value::Bool(_))) => ids.push(format!("{name}={v}")),
            _ => {}
        }
    }
    let message = one_line(&redact(&e.message));
    let line = format!(
        "{} {} {} {} {} {{{}}}",
        e.ts,
        e.level.name().to_ascii_lowercase(),
        e.target,
        e.event,
        message,
        ids.join(" ")
    );
    cut_chars(&line, MAX_LINE_CHARS)
}

fn matches_component(target: &str, component: &str) -> bool {
    target.starts_with(component)
        || target
            .strip_prefix("townhall::")
            .is_some_and(|short| short.starts_with(component))
}

/// The newest entries that pass the filters, as compact lines, within the output cap.
pub fn read_logs(dir: &Path, now_ms: i64, args: &ReadArgs) -> Result<ReadOutput, String> {
    let min = level_arg(args.level.as_deref())?;
    let since = parse_since(args.since.as_deref().unwrap_or(DEFAULT_SINCE), now_ms)?;
    let limit = args.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    let grep = args.grep.as_deref().map(str::to_lowercase);
    let entries = read_entries(dir, since, |e| {
        e.level >= min
            && args
                .component
                .as_deref()
                .is_none_or(|c| matches_component(&e.target, c))
            && args.event.as_deref().is_none_or(|ev| e.event == ev)
            && args
                .task
                .as_deref()
                .is_none_or(|t| e.field("task") == Some(t))
            && grep.as_deref().is_none_or(|g| {
                e.message.to_lowercase().contains(g)
                    || e.field("error")
                        .is_some_and(|x| x.to_lowercase().contains(g))
            })
    });

    let matched = entries.len();
    let mut lines = Vec::new();
    // `{"lines":[],"count":999,"truncated":false}` plus slack.
    let mut bytes = 64;
    for entry in entries.iter().take(limit) {
        let line = format_line(entry);
        let cost = serde_json::to_string(&line).map_or(line.len() + 2, |s| s.len()) + 1;
        if bytes + cost > MAX_OUTPUT_BYTES {
            break;
        }
        bytes += cost;
        lines.push(line);
    }
    Ok(ReadOutput {
        count: lines.len(),
        truncated: lines.len() < matched.min(limit),
        lines,
    })
}

/// The distinct fingerprints of the warnings and errors that name `task`, since `since_ms`.
pub fn task_fingerprints(dir: &Path, since_ms: i64, task: &str) -> Vec<String> {
    let entries = read_entries(dir, since_ms, |e| {
        e.level >= Level::Warn && e.field("task") == Some(task)
    });
    let mut prints: Vec<String> = entries
        .iter()
        .map(|e| {
            let ids: Vec<(&str, &str)> = ID_FIELDS
                .iter()
                .filter_map(|name| e.field(name).map(|v| (*name, v)))
                .collect();
            fingerprint(&e.target, &e.event, &e.message, &ids)
        })
        .collect();
    prints.sort();
    prints.dedup();
    prints
}

/// Counts, panics and the top recurring groups since `args.since`, within the output cap.
pub fn log_summary(dir: &Path, now_ms: i64, args: &SummaryArgs) -> Result<Summary, String> {
    let min = level_arg(args.level.as_deref())?;
    let since_arg = args.since.as_deref().unwrap_or(DEFAULT_SINCE);
    let since = parse_since(since_arg, now_ms)?;
    let top = args.top.unwrap_or(DEFAULT_TOP).clamp(1, MAX_TOP);
    let entries = read_entries(dir, since, |e| e.level >= min);

    let mut by_level: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_component: HashMap<&str, usize> = HashMap::new();
    let mut panics = Vec::new();
    let mut groups: HashMap<String, Group> = HashMap::new();
    // Entries are newest first: the first of a group is its newest and gives the sample.
    for e in &entries {
        *by_level.entry(e.level.name().to_string()).or_default() += 1;
        *by_component.entry(e.target.as_str()).or_default() += 1;
        if e.event == "panic" {
            panics.push(PanicEntry {
                ts: e.ts.clone(),
                message: cut_chars(&one_line(&redact(&e.message)), MAX_LINE_CHARS),
                location: e.field("location").map(|l| cut_chars(&one_line(l), 120)),
                source: e.source,
            });
        }
        let ids: Vec<(&str, &str)> = ID_FIELDS
            .iter()
            .filter_map(|name| e.field(name).map(|v| (*name, v)))
            .collect();
        let key = fingerprint(&e.target, &e.event, &e.message, &ids);
        let group = groups.entry(key.clone()).or_insert_with(|| Group {
            fingerprint: key,
            count: 0,
            target: e.target.clone(),
            event: e.event.clone(),
            first_seen: e.ts.clone(),
            last_seen: e.ts.clone(),
            tasks: Vec::new(),
            sample: cut_chars(&one_line(&redact(&e.message)), MAX_SAMPLE_CHARS),
        });
        group.count += 1;
        if e.ts_ms < parse_rfc3339(&group.first_seen).unwrap_or(i64::MAX) {
            group.first_seen = e.ts.clone();
        }
        if let Some(task) = e.field("task") {
            if group.tasks.len() < MAX_TASKS_PER_GROUP && !group.tasks.iter().any(|t| t == task) {
                group.tasks.push(task.to_string());
            }
        }
    }

    let mut by_component: Vec<ComponentCount> = by_component
        .into_iter()
        .map(|(component, count)| ComponentCount {
            component: component.to_string(),
            count,
        })
        .collect();
    by_component.sort_by(|a, b| b.count.cmp(&a.count).then(a.component.cmp(&b.component)));
    let mut groups: Vec<Group> = groups.into_values().collect();
    groups.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then(b.last_seen.cmp(&a.last_seen))
            .then(a.fingerprint.cmp(&b.fingerprint))
    });
    groups.truncate(top);

    let mut summary = Summary {
        since: since_arg.to_string(),
        level: min.name().to_ascii_lowercase(),
        total: entries.len(),
        by_level,
        by_component,
        panics,
        groups,
        truncated: false,
    };
    cap_summary(&mut summary);
    Ok(summary)
}

/// Drops the least frequent groups, then the oldest panics and components, until the JSON
/// fits `MAX_OUTPUT_BYTES`.
fn cap_summary(summary: &mut Summary) {
    let size = |s: &Summary| serde_json::to_string(s).map_or(usize::MAX, |j| j.len());
    while size(summary) > MAX_OUTPUT_BYTES {
        summary.truncated = true;
        if summary.groups.pop().is_some() {
            continue;
        }
        if summary.panics.pop().is_some() {
            continue;
        }
        if summary.by_component.pop().is_none() {
            break;
        }
    }
}
