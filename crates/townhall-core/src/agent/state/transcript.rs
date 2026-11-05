//! Where Claude's turn stands, read from the end of its transcript (`~/.claude/projects`).
//! Only the last assistant message and what came after it matter, so only the file's tail
//! is read, growing the window while that message is not in it.

use std::collections::HashSet;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::Mutex;

use serde_json::Value;

use super::question::{shorten, with_options, Item};
use super::stamp;
use crate::steps::{parse_transcript, StepResult};

const FIRST_WINDOW: u64 = 64 * 1024;
const MAX_WINDOW: u64 = 16 * 1024 * 1024;
const ASK_TOOL: &str = "AskUserQuestion";
const INTERRUPTED: &str = "[Request interrupted by user";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Turn {
    /// Streaming, thinking, or about to answer a prompt or a tool result.
    Busy,
    /// A tool call without a result: it runs, or waits for permission. `call` describes it.
    Pending { call: String },
    /// `AskUserQuestion` waits for an answer: the question with its options.
    Asking { question: String },
    /// The turn ended; `text` is what the last message said, `at` when it ended (unix
    /// milliseconds, from the line that ended it).
    Idle { text: String, at: Option<i64> },
}

/// What the scan of one window collected, newest line first.
#[derive(Default)]
struct Scan {
    results: HashSet<String>,
    ended: bool,
    message: Option<String>,
    stop: Option<String>,
    texts: Vec<String>,
    pending: Option<(String, String, Value)>,
    api_error: bool,
    /// When the newest line that ends or makes the turn was written.
    at: Option<i64>,
}

fn text<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

fn blocks(v: &Value) -> &[Value] {
    v.pointer("/message/content")
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

/// A prompt you typed (not a tool result, not text townhall or Claude injected): its text.
fn prompt_text(v: &Value) -> Option<String> {
    if v.get("isMeta").and_then(Value::as_bool) == Some(true) {
        return None;
    }
    if let Some(s) = v.pointer("/message/content").and_then(Value::as_str) {
        return Some(s.to_string());
    }
    let all = blocks(v);
    if all.is_empty() || all.iter().any(|b| text(b, "type") == Some("tool_result")) {
        return None;
    }
    let texts: Vec<&str> = all.iter().filter_map(|b| text(b, "text")).collect();
    Some(texts.join("\n"))
}

impl Scan {
    fn stamp(&mut self, v: &Value) {
        if self.at.is_none() {
            self.at = text(v, "timestamp").and_then(stamp::millis);
        }
    }

    /// Takes one line, newest first. `Some` ends the scan early with that answer.
    fn take(&mut self, v: &Value) -> Option<Option<Turn>> {
        if v.get("isSidechain").and_then(Value::as_bool) == Some(true) {
            return None;
        }
        match text(v, "type")? {
            "system" if self.message.is_none() => {
                if text(v, "subtype") == Some("turn_duration") {
                    self.ended = true;
                    self.stamp(v);
                }
                None
            }
            "user" => self.user(v),
            "assistant" => self.assistant(v),
            _ => None,
        }
    }

    /// Results count wherever they are; a prompt only ends the scan after the last message.
    fn user(&mut self, v: &Value) -> Option<Option<Turn>> {
        for b in blocks(v) {
            if let Some(id) = text(b, "tool_use_id") {
                self.results.insert(id.to_string());
            }
        }
        if self.message.is_some() {
            return None;
        }
        let prompt = prompt_text(v)?;
        Some(Some(if prompt.trim_start().starts_with(INTERRUPTED) {
            Turn::Idle {
                text: String::new(),
                at: text(v, "timestamp").and_then(stamp::millis),
            }
        } else {
            Turn::Busy
        }))
    }

    fn assistant(&mut self, v: &Value) -> Option<Option<Turn>> {
        let id = v.pointer("/message/id").and_then(Value::as_str);
        match (&self.message, id) {
            (None, _) => {
                self.stamp(v);
                self.message = Some(id.unwrap_or_default().to_string());
                self.stop = v
                    .pointer("/message/stop_reason")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                self.api_error = v.get("isApiErrorMessage").and_then(Value::as_bool) == Some(true);
            }
            (Some(seen), Some(id)) if seen == id => {}
            _ => return Some(None),
        }
        for b in blocks(v).iter().rev() {
            match text(b, "type") {
                Some("text") => self
                    .texts
                    .push(text(b, "text").unwrap_or_default().to_string()),
                Some("tool_use") => self.note_call(b),
                _ => {}
            }
        }
        None
    }

    fn note_call(&mut self, b: &Value) {
        let Some(id) = text(b, "id") else { return };
        if self.pending.is_none() && !self.results.contains(id) {
            let name = text(b, "name").unwrap_or_default().to_string();
            let input = b.get("input").cloned().unwrap_or(Value::Null);
            self.pending = Some((id.to_string(), name, input));
        }
    }
}

/// The first `AskUserQuestion` question with its options, and how many more follow.
fn asked(input: &Value) -> String {
    let questions = input
        .get("questions")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    let Some(first) = questions.first() else {
        return String::from("Claude has a question");
    };
    let options: Vec<Item> = first
        .get("options")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .filter_map(|o| text(o, "label"))
        .zip(1..)
        .map(|(label, number)| Item {
            number,
            label: label.to_string(),
        })
        .collect();
    let question = text(first, "question").unwrap_or("Claude has a question");
    let more = match questions.len() - 1 {
        0 => String::new(),
        1 => "\n+1 more question".to_string(),
        n => format!("\n+{n} more questions"),
    };
    format!("{}{more}", with_options(question, &options))
}

/// `Tool: what` for a call still running, from 2.7's step summaries.
fn describe(window: &str, id: &str, tool: &str) -> String {
    let summary = parse_transcript(window)
        .into_iter()
        .find(|s| s.id == id && s.result == StepResult::Running)
        .map(|s| s.summary)
        .filter(|s| s != tool);
    match summary {
        Some(s) => shorten(&format!("{tool}: {s}")),
        None => tool.to_string(),
    }
}

/// Where the turn stands, from a window of whole lines. `None` when the window has
/// neither an assistant message nor a prompt.
pub(crate) fn parse(window: &str) -> Option<Turn> {
    let mut scan = Scan::default();
    for line in window.lines().rev() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if let Some(done) = scan.take(&v) {
            if scan.message.is_none() {
                return done;
            }
            break;
        }
    }
    scan.message.as_ref()?;
    Some(match scan.pending {
        Some((_, name, input)) if name == ASK_TOOL => Turn::Asking {
            question: asked(&input),
        },
        Some((id, name, _)) => Turn::Pending {
            call: describe(window, &id, &name),
        },
        None if scan.ended || scan.api_error || scan.stop.as_deref() == Some("end_turn") => {
            let texts: Vec<String> = scan.texts.into_iter().rev().collect();
            Turn::Idle {
                text: texts.join("\n"),
                at: scan.at,
            }
        }
        None => Turn::Busy,
    })
}

fn window_at(file: &mut File, start: u64) -> std::io::Result<String> {
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = vec![];
    file.read_to_end(&mut bytes)?;
    let text = String::from_utf8_lossy(&bytes).into_owned();
    if start == 0 {
        return Ok(text);
    }
    Ok(text
        .split_once('\n')
        .map(|(_, rest)| rest.to_string())
        .unwrap_or_default())
}

/// Longest start of a tool's error text a log line carries, in chars.
const ERROR_CHARS: usize = 200;
/// How many tool results are remembered as logged; the oldest half goes when it is full.
const LOGGED_CAP: usize = 4096;

static LOGGED: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// True the first time `id` is seen, so a window read again does not log a result twice.
fn first_time(id: &str) -> bool {
    let mut logged = LOGGED.lock().unwrap_or_else(|p| p.into_inner());
    if logged.iter().any(|seen| seen == id) {
        return false;
    }
    if logged.len() >= LOGGED_CAP {
        logged.drain(..LOGGED_CAP / 2);
    }
    logged.push(id.to_string());
    true
}

/// The text of a tool result: a string, or the text blocks of a list.
fn result_text(block: &Value) -> String {
    match block.get("content") {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|i| text(i, "text"))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// Claude's wording when a tool call is refused by the permission rules or by the user.
fn denial(error: &str) -> bool {
    error.starts_with("Permission to use")
        || error.contains("has been denied")
        || error.contains("doesn't want to proceed with this tool use")
}

/// Logs each tool result in `window` that failed: `agent.permission_denied` (tool name
/// only) for a refused call, else `agent.tool_error` (tool name and the redacted start of the
/// error). Tool inputs and the rest of the transcript are never logged.
fn report(window: &str, agent: &str) {
    let values: Vec<Value> = window
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    let mut tools = std::collections::HashMap::new();
    for v in &values {
        for b in blocks(v) {
            if let (Some(id), Some("tool_use")) = (text(b, "id"), text(b, "type")) {
                tools.insert(id, text(b, "name").unwrap_or_default());
            }
        }
    }
    for v in &values {
        for b in blocks(v) {
            let failed = b.get("is_error").and_then(Value::as_bool) == Some(true);
            let Some(id) = text(b, "tool_use_id").filter(|_| failed) else {
                continue;
            };
            if text(b, "type") != Some("tool_result") || !first_time(id) {
                continue;
            }
            let tool = tools.get(id).copied().unwrap_or("unknown");
            let error = result_text(b);
            if denial(&error) {
                tracing::info!(target: "townhall::agent", event = "agent.permission_denied",
                    agent = %agent, tool = %tool, "tool call denied");
            } else {
                let error = crate::redact::redact(&error)
                    .chars()
                    .take(ERROR_CHARS)
                    .collect::<String>();
                tracing::info!(target: "townhall::agent", event = "agent.tool_error",
                    agent = %agent, tool = %tool, error = %error, "tool call failed");
            }
        }
    }
}

/// Reads the transcript's tail until the turn can be told. `None` when nothing in it does.
pub(crate) fn read(path: &Path) -> std::io::Result<Option<Turn>> {
    read_with(path, None)
}

/// Like [`read`], and logs the failed tool results it passes for `agent`.
pub(crate) fn read_as(path: &Path, agent: &str) -> std::io::Result<Option<Turn>> {
    read_with(path, Some(agent))
}

fn read_with(path: &Path, agent: Option<&str>) -> std::io::Result<Option<Turn>> {
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    let mut size = FIRST_WINDOW;
    loop {
        let start = len.saturating_sub(size);
        let window = window_at(&mut file, start)?;
        if let Some(turn) = parse(&window) {
            if let Some(agent) = agent {
                report(&window, agent);
            }
            return Ok(Some(turn));
        }
        if start == 0 || size >= MAX_WINDOW {
            return Ok(None);
        }
        size *= 4;
    }
}

#[cfg(test)]
mod tests;
