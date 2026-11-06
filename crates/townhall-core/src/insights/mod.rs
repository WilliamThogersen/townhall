//! `conversation_insights`: recurring patterns across every conversation, found without a
//! model. Pure and synchronous: the caller loads the data from the store (`mcp/insights.rs`)
//! and this module counts, ranks, caps and redacts it.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::logs::{fingerprint, parse_rfc3339, parse_since};
use crate::model::{AgentEvent, EntryBody, Handoff, Task, ThreadEntry};
use crate::redact::redact;

#[cfg(test)]
mod tests;

/// The whole output stays under this many bytes of JSON; the lowest ranked patterns go first.
pub const MAX_OUTPUT_BYTES: usize = 24 * 1024;
pub const DEFAULT_TOP: usize = 10;
pub const MAX_TOP: usize = 30;
pub const DEFAULT_SINCE: &str = "30d";
/// A pattern with fewer occurrences is not recurring.
pub const MIN_COUNT: usize = 2;
pub const MAX_SNIPPET_CHARS: usize = 160;
pub const MAX_SAMPLES: usize = 3;
pub const MAX_IDS: usize = 5;
/// A task chain longer than this is cut.
const MAX_CHAIN: usize = 8;

/// One conversation's thread.
#[derive(Debug, Clone)]
pub struct ConversationData {
    pub id: String,
    pub entries: Vec<ThreadEntry>,
}

/// One task with its handoffs and when it ended (`Store::task_ended_times`).
#[derive(Debug, Clone)]
pub struct TaskData {
    pub task: Task,
    pub handoffs: Vec<Handoff>,
    pub ended_at: Option<String>,
}

/// Everything the analysis reads, from every conversation.
#[derive(Debug, Clone, Default)]
pub struct InsightData {
    pub conversations: Vec<ConversationData>,
    pub tasks: Vec<TaskData>,
}

/// `conversation_insights` arguments.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InsightsArgs {
    /// A duration like 7d or an RFC 3339 time; default 30 days.
    pub since: Option<String>,
    /// Patterns to return; default 10, at most 30.
    pub top: Option<usize>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Counts {
    pub conversations: usize,
    pub tasks: usize,
    pub user_messages: usize,
    pub needs_you: usize,
    pub agent_failed: usize,
    pub agent_sent: usize,
    pub handoffs: usize,
    pub errors: usize,
    /// Tasks per agent type, by their state.
    pub tasks_by_agent: BTreeMap<String, BTreeMap<String, usize>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Pattern {
    pub kind: &'static str,
    pub pattern: String,
    pub count: usize,
    pub first_seen: String,
    pub last_seen: String,
    pub samples: Vec<String>,
    pub conversations: Vec<String>,
    pub tasks: Vec<String>,
    pub fingerprint: String,
    pub suggested_automation: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Insights {
    pub since: String,
    pub counts: Counts,
    pub patterns: Vec<Pattern>,
    /// True when more patterns matched than `patterns` holds (`top` or the size cap).
    pub truncated: bool,
}

/// A user phrase or event text with ids, numbers and paths stripped, lowercased, so
/// repeats of the same request match.
pub fn normalize_phrase(text: &str) -> String {
    redact(text)
        .to_lowercase()
        .split_whitespace()
        .filter(|w| !w.contains(|c: char| c.is_ascii_digit() || c == '/' || c == '\\'))
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// What a pattern's `suggested_automation` is built from.
enum Hint {
    Request,
    NeedsYou,
    Failed,
    ManualStep,
    Handoff {
        from: String,
        to: String,
        has_reason: bool,
    },
    Chain(Vec<String>),
    Error,
}

impl Hint {
    fn suggestion(&self) -> String {
        match self {
            Hint::Request => "new agent type or planner prompt: the same request keeps coming back, so save it as an agent type for that role or add a rule to the planner prompt".into(),
            Hint::NeedsYou => "ask-less default: put the answer to this question in the agent type's prompt or give it a default so it stops asking".into(),
            Hint::Failed => "prompt or tool change: agents fail the same way, so fix the cause in the agent type's prompt or give it the tool it lacks".into(),
            Hint::ManualStep => "auto-chain or tool: this step is typed into agents by hand, so add it to the agent type's prompt or chain a task that does it".into(),
            Hint::Handoff { from, to, has_reason: false } => format!("auto-chain after {from}: create the {to} task automatically instead of handing back by hand"),
            Hint::Handoff { from, to, has_reason: true } => format!("prompt change for {from}: it hands back to {to} for the same reason each time, so cover that in its prompt"),
            Hint::Chain(types) => format!("auto-chain after {}: start {} automatically as one chain", types[0], types[1..].join(", then ")),
            Hint::Error => "tool change in townhall: its session fails the same way each time, so create a task to fix that cause (log_summary may show it)".into(),
        }
    }
}

struct Group {
    kind: &'static str,
    pattern: String,
    hint: Hint,
    count: usize,
    first: (i64, String),
    last: (i64, String),
    samples: Vec<String>,
    conversations: BTreeSet<String>,
    tasks: BTreeSet<String>,
}

/// One occurrence of a pattern.
struct Seen<'a> {
    kind: &'static str,
    pattern: String,
    hint: Hint,
    at: &'a str,
    at_ms: i64,
    sample: &'a str,
    conversation: Option<&'a str>,
    task: Option<&'a str>,
}

#[derive(Default)]
struct Groups(BTreeMap<(&'static str, String), Group>);

impl Groups {
    fn add(&mut self, s: Seen) {
        let group = self
            .0
            .entry((s.kind, s.pattern.clone()))
            .or_insert_with(|| Group {
                kind: s.kind,
                pattern: s.pattern,
                hint: s.hint,
                count: 0,
                first: (s.at_ms, s.at.to_string()),
                last: (s.at_ms, s.at.to_string()),
                samples: vec![],
                conversations: BTreeSet::new(),
                tasks: BTreeSet::new(),
            });
        group.count += 1;
        if s.at_ms < group.first.0 {
            group.first = (s.at_ms, s.at.to_string());
        }
        if s.at_ms > group.last.0 {
            group.last = (s.at_ms, s.at.to_string());
        }
        let sample = snippet(s.sample);
        if group.samples.len() < MAX_SAMPLES
            && !sample.is_empty()
            && !group.samples.contains(&sample)
        {
            group.samples.push(sample);
        }
        group.conversations.extend(s.conversation.map(String::from));
        group.tasks.extend(s.task.map(String::from));
    }
}

/// Redacted, on one line and at most [`MAX_SNIPPET_CHARS`] chars.
fn snippet(text: &str) -> String {
    let line = redact(text)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if line.chars().count() <= MAX_SNIPPET_CHARS {
        return line;
    }
    let cut: String = line.chars().take(MAX_SNIPPET_CHARS - 1).collect();
    format!("{cut}…")
}

/// The text of an agent entry without the agent's own name, so the same message about two
/// tasks matches.
fn normalize_about(text: &str, name: &str) -> String {
    normalize_phrase(&if name.is_empty() {
        text.to_string()
    } else {
        text.replace(name, " ")
    })
}

fn ids(set: &BTreeSet<String>) -> Vec<String> {
    set.iter()
        .take(MAX_IDS)
        .map(|s| redact(s).into_owned())
        .collect()
}

/// Counts, ranks and caps the recurring patterns of `data` since `args.since`.
pub fn analyze(data: &InsightData, now_ms: i64, args: &InsightsArgs) -> Result<Insights, String> {
    let since = args.since.as_deref().unwrap_or(DEFAULT_SINCE);
    let since_ms = parse_since(since, now_ms)?;
    let top = args.top.unwrap_or(DEFAULT_TOP).clamp(1, MAX_TOP);
    let in_window = |at: &str| parse_rfc3339(at).filter(|ms| *ms >= since_ms);

    let mut counts = Counts {
        conversations: data.conversations.len(),
        ..Counts::default()
    };
    let mut groups = Groups::default();
    for conversation in &data.conversations {
        for e in &conversation.entries {
            let Some(at_ms) = in_window(&e.at) else {
                continue;
            };
            let seen = |kind, pattern: String, hint, sample, task| Seen {
                kind,
                pattern,
                hint,
                at: &e.at,
                at_ms,
                sample,
                conversation: Some(&conversation.id),
                task,
            };
            let found = match &e.body {
                EntryBody::You {
                    text,
                    reply_to: None,
                    ..
                } => {
                    counts.user_messages += 1;
                    Some(seen(
                        "user_request",
                        normalize_phrase(text),
                        Hint::Request,
                        text,
                        None,
                    ))
                }
                EntryBody::You { .. } => {
                    counts.user_messages += 1;
                    None
                }
                EntryBody::NeedsYou {
                    agent, question, ..
                } => {
                    counts.needs_you += 1;
                    Some(seen(
                        "needs_you",
                        normalize_about(question, agent),
                        Hint::NeedsYou,
                        question,
                        Some(agent),
                    ))
                }
                EntryBody::Agent {
                    agent,
                    event: AgentEvent::Failed,
                    text,
                } => {
                    counts.agent_failed += 1;
                    Some(seen(
                        "agent_failed",
                        normalize_about(text, agent),
                        Hint::Failed,
                        text,
                        Some(agent),
                    ))
                }
                EntryBody::Agent {
                    agent,
                    event: AgentEvent::Sent,
                    text,
                } => {
                    counts.agent_sent += 1;
                    Some(seen(
                        "manual_step",
                        normalize_about(text, agent),
                        Hint::ManualStep,
                        text,
                        Some(agent),
                    ))
                }
                EntryBody::Error { message } => {
                    counts.errors += 1;
                    Some(seen(
                        "townhall_error",
                        normalize_phrase(message),
                        Hint::Error,
                        message,
                        None,
                    ))
                }
                _ => None,
            };
            if let Some(s) = found.filter(|s| !s.pattern.is_empty()) {
                groups.add(s);
            }
        }
    }

    let by_name: HashMap<&str, &TaskData> = data
        .tasks
        .iter()
        .map(|t| (t.task.name.as_str(), t))
        .collect();
    let depended_on: HashSet<&str> = data
        .tasks
        .iter()
        .flat_map(|t| t.task.after.iter().map(String::as_str))
        .collect();
    for t in &data.tasks {
        let task = &t.task;
        for h in &t.handoffs {
            let Some(at_ms) = in_window(&h.at) else {
                continue;
            };
            counts.handoffs += 1;
            let reason = normalize_phrase(&h.reason);
            let pattern = if reason.is_empty() {
                format!("{} -> {}", h.from, h.to)
            } else {
                format!("{} -> {}: {reason}", h.from, h.to)
            };
            groups.add(Seen {
                kind: "handoff",
                hint: Hint::Handoff {
                    from: h.from.clone(),
                    to: h.to.clone(),
                    has_reason: !reason.is_empty(),
                },
                pattern,
                at: &h.at,
                at_ms,
                sample: if h.reason.is_empty() {
                    &h.summary
                } else {
                    &h.reason
                },
                conversation: Some(&task.conversation_id),
                task: Some(&task.name),
            });
        }
        let Some(at_ms) = in_window(&task.created_at) else {
            continue;
        };
        counts.tasks += 1;
        let state = serde_json::to_value(task.state)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default();
        *counts
            .tasks_by_agent
            .entry(task.agent.clone())
            .or_default()
            .entry(state)
            .or_default() += 1;
        if depended_on.contains(task.name.as_str()) {
            continue;
        }
        let chain = chain_of(t, &by_name);
        if chain.len() >= 2 {
            groups.add(Seen {
                kind: "task_chain",
                pattern: chain.join(" -> "),
                hint: Hint::Chain(chain),
                at: &task.created_at,
                at_ms,
                sample: &task.title,
                conversation: Some(&task.conversation_id),
                task: Some(&task.name),
            });
        }
    }

    let mut found: Vec<Group> = groups
        .0
        .into_values()
        .filter(|g| g.count >= MIN_COUNT)
        .collect();
    found.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then(b.last.0.cmp(&a.last.0))
            .then(a.kind.cmp(b.kind))
            .then(a.pattern.cmp(&b.pattern))
    });
    let mut truncated = found.len() > top;
    found.truncate(top);
    let mut out = Insights {
        since: since.to_string(),
        counts,
        patterns: found.iter().map(finish).collect(),
        truncated,
    };
    while out.patterns.len() > 1 && encoded_len(&out) > MAX_OUTPUT_BYTES {
        out.patterns.pop();
        truncated = true;
        out.truncated = truncated;
    }
    Ok(out)
}

fn encoded_len(out: &Insights) -> usize {
    serde_json::to_string(out).map_or(usize::MAX, |s| s.len())
}

fn finish(g: &Group) -> Pattern {
    let pattern = snippet(&g.pattern);
    Pattern {
        kind: g.kind,
        fingerprint: fingerprint("townhall::insights", g.kind, &pattern, &[]),
        pattern,
        count: g.count,
        first_seen: g.first.1.clone(),
        last_seen: g.last.1.clone(),
        samples: g.samples.clone(),
        conversations: ids(&g.conversations),
        tasks: ids(&g.tasks),
        suggested_automation: redact(&g.hint.suggestion()).into_owned(),
    }
}

/// The agent types a task went through: the type it began as, then each type it was handed
/// to. A hand back sets `task.agent` to the new type, so the first `from` is the start.
fn stages(t: &TaskData) -> Vec<String> {
    let start = t
        .handoffs
        .first()
        .map_or(&t.task.agent, |h| &h.from)
        .clone();
    std::iter::once(start)
        .chain(t.handoffs.iter().map(|h| h.to.clone()))
        .collect()
}

/// The agent types a leaf task went through: the stages of its `after` lineage (first parent
/// each step), oldest first, then its own.
fn chain_of(leaf: &TaskData, by_name: &HashMap<&str, &TaskData>) -> Vec<String> {
    let mut lineage = vec![];
    let mut seen = HashSet::new();
    let mut at = Some(leaf);
    while let Some(t) = at.filter(|t| seen.insert(t.task.name.clone())) {
        lineage.push(t);
        at = t
            .task
            .after
            .first()
            .and_then(|name| by_name.get(name.as_str()).copied());
    }
    let mut types: Vec<String> = lineage.into_iter().rev().flat_map(stages).collect();
    types.truncate(MAX_CHAIN);
    types
}
