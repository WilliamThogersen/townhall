use std::collections::{HashMap, HashSet};

use serde_json::Value;

use super::summary::summarize;
use super::{Step, StepResult};

/// A `tool_use` block in an assistant message.
struct Call {
    id: String,
    tool: String,
    summary: String,
    at: String,
}

/// A `tool_result` block: when it arrived and whether it failed.
struct Outcome {
    at: Option<String>,
    error: bool,
}

enum Event {
    Call(Call),
    Outcome(String, Outcome),
}

pub fn transcript_tokens(jsonl: &str) -> u64 {
    jsonl
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter_map(|line| {
            let message = line.get("message")?;
            let id = message.get("id")?.as_str()?.to_string();
            let usage = message.get("usage")?;
            let count = |key: &str| usage.get(key).and_then(Value::as_u64).unwrap_or(0);
            Some((id, count("input_tokens") + count("output_tokens")))
        })
        .collect::<HashMap<String, u64>>()
        .values()
        .sum()
}

pub fn transcript_model(jsonl: &str) -> Option<String> {
    jsonl
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter_map(|line| {
            let model = line.get("message")?.get("model")?.as_str()?;
            (!model.is_empty() && !model.starts_with('<')).then(|| model.to_string())
        })
        .last()
}

/// One step per `tool_use` in an assistant message, paired with its `tool_result` by id
/// and ordered by start. Lines that do not parse, and sidechain lines, are skipped.
pub fn parse_transcript(jsonl: &str) -> Vec<Step> {
    let events: Vec<Event> = jsonl
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .flat_map(|line| events_in(&line))
        .collect();
    let outcomes = first_outcomes(&events);
    let mut seen = HashSet::new();
    let mut steps: Vec<Step> = events
        .iter()
        .filter_map(|e| match e {
            Event::Call(call) if seen.insert(call.id.as_str()) => Some(step(call, &outcomes)),
            _ => None,
        })
        .collect();
    steps.sort_by(|a, b| a.started_at.cmp(&b.started_at));
    steps
}

/// The first result per tool_use id; later ones are replays.
fn first_outcomes(events: &[Event]) -> HashMap<&str, &Outcome> {
    events.iter().fold(HashMap::new(), |mut acc, e| {
        if let Event::Outcome(id, outcome) = e {
            acc.entry(id.as_str()).or_insert(outcome);
        }
        acc
    })
}

fn step(call: &Call, outcomes: &HashMap<&str, &Outcome>) -> Step {
    let outcome = outcomes.get(call.id.as_str());
    let result = match outcome {
        None => StepResult::Running,
        Some(o) if o.error => StepResult::Error,
        Some(_) => StepResult::Ok,
    };
    Step {
        id: call.id.clone(),
        tool: call.tool.clone(),
        summary: call.summary.clone(),
        started_at: call.at.clone(),
        ended_at: outcome.and_then(|o| o.at.clone()),
        result,
    }
}

fn events_in(line: &Value) -> Vec<Event> {
    if line.get("isSidechain").and_then(Value::as_bool) == Some(true) {
        return vec![];
    }
    let Some(blocks) = line.pointer("/message/content").and_then(Value::as_array) else {
        return vec![];
    };
    let at = text(line, "timestamp");
    let cwd = text(line, "cwd");
    let assistant = text(line, "type") == Some("assistant");
    blocks
        .iter()
        .filter_map(|block| match text(block, "type") {
            Some("tool_use") if assistant => call(block, at?, cwd).map(Event::Call),
            Some("tool_result") => outcome(block, at),
            _ => None,
        })
        .collect()
}

fn call(block: &Value, at: &str, cwd: Option<&str>) -> Option<Call> {
    let id = text(block, "id").filter(|s| !s.is_empty())?;
    let tool = text(block, "name").filter(|s| !s.is_empty())?;
    Some(Call {
        id: id.to_string(),
        tool: tool.to_string(),
        summary: summarize(tool, block.get("input"), cwd),
        at: at.to_string(),
    })
}

fn outcome(block: &Value, at: Option<&str>) -> Option<Event> {
    let id = text(block, "tool_use_id")?;
    let error = block.get("is_error").and_then(Value::as_bool) == Some(true);
    Some(Event::Outcome(
        id.to_string(),
        Outcome {
            at: at.map(str::to_string),
            error,
        },
    ))
}

fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}
