use rusqlite::types::Value;
use rusqlite::{params_from_iter, Connection};
use serde::{Deserialize, Serialize};

use super::codec::{name_of, parse_name};
use super::migrate::Migration;
use super::Store;
use crate::model::{TaskState, TASK_BRANCH_PREFIX};
use crate::{clock, Error, Result};

pub const MIGRATIONS: &[Migration] = &[
    Migration {
        id: 1000,
        name: "1000_history_index",
        sql: include_str!("migrations/1000_history_index.sql"),
    },
    Migration {
        id: 1001,
        name: "1001_default_history_tools",
        sql: include_str!("migrations/1001_default_history_tools.sql"),
    },
];

const SNIPPET_WORDS: u32 = 14;
const OPERATORS: [&str; 4] = ["AND", "OR", "NOT", "NEAR"];
const SYNTAX: [char; 8] = ['"', '(', ')', ':', '*', '^', '{', '}'];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoryKind {
    Message,
    Task,
    AgentReport,
    NeedsYou,
}

impl HistoryKind {
    pub fn name(self) -> String {
        name_of(&self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryQuery {
    pub query: String,
    pub repo: Option<String>,
    pub since_ms: Option<i64>,
    pub kinds: Vec<HistoryKind>,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryHit {
    pub kind: HistoryKind,
    pub conversation_id: String,
    pub title: String,
    pub task: Option<String>,
    pub agent: Option<String>,
    pub state: Option<TaskState>,
    pub branch: Option<String>,
    pub pr_number: Option<u64>,
    pub pr_url: Option<String>,
    pub at: String,
    pub snippet: String,
}

fn match_expression(text: &str) -> Option<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return None;
    }
    let plain = words.iter().all(|w| {
        !w.contains(SYNTAX)
            && !w.starts_with(['-', '+'])
            && !OPERATORS.contains(w)
            && !w.starts_with("NEAR")
    });
    Some(match plain {
        true => words
            .iter()
            .map(|w| format!("\"{w}\""))
            .collect::<Vec<_>>()
            .join(" "),
        false => words.join(" "),
    })
}

fn bad_syntax(e: rusqlite::Error) -> Error {
    match &e {
        rusqlite::Error::SqliteFailure(_, Some(message))
            if message.starts_with("fts5:")
                || message.starts_with("no such column")
                || message.starts_with("unterminated string") =>
        {
            Error::InvalidInput(format!(
                "cannot read the search text ({message}): put words with punctuation in double quotes"
            ))
        }
        _ => e.into(),
    }
}

fn search(conn: &Connection, query: &HistoryQuery) -> Result<Vec<HistoryHit>> {
    let Some(expression) = match_expression(&query.query) else {
        return Ok(vec![]);
    };
    let mut filters = String::new();
    let mut params = vec![Value::Text(expression)];
    if let Some(ms) = query.since_ms {
        params.push(Value::Text(clock::from_unix_millis(ms)));
        filters.push_str(&format!(" and history_fts.at >= ?{}", params.len()));
    }
    if let Some(repo) = &query.repo {
        params.push(Value::Text(repo.clone()));
        let n = params.len();
        filters.push_str(&format!(
            " and (lower(c.linked_repo) = lower(?{n}) or exists (
                select 1 from tasks r where r.conversation_id = c.id and lower(r.repo) = lower(?{n})))"
        ));
    }
    if !query.kinds.is_empty() {
        let kinds: Vec<String> = query
            .kinds
            .iter()
            .map(|k| format!("'{}'", k.name()))
            .collect();
        filters.push_str(&format!(" and history_fts.kind in ({})", kinds.join(", ")));
    }
    params.push(Value::Integer(i64::try_from(query.limit).unwrap_or(i64::MAX)));
    let sql = format!(
        "select history_fts.kind, history_fts.conversation_id, c.title, history_fts.task,
                t.agent, t.state, t.pr_number, t.pr_url, history_fts.at,
                snippet(history_fts, 0, '[', ']', '\u{2026}', {SNIPPET_WORDS})
         from history_fts
         join conversations c on c.id = history_fts.conversation_id
         left join tasks t on t.name = history_fts.task
         where history_fts match ?1{filters}
         order by bm25(history_fts), history_fts.at desc
         limit ?{}",
        params.len()
    );
    let mut stmt = conn.prepare(&sql).map_err(bad_syntax)?;
    let rows = stmt
        .query_map(params_from_iter(params), |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, Option<u64>>(6)?,
                r.get::<_, Option<String>>(7)?,
                r.get::<_, String>(8)?,
                r.get::<_, String>(9)?,
            ))
        })
        .map_err(bad_syntax)?;
    let mut hits = vec![];
    for row in rows {
        let (kind, conversation_id, title, task, agent, state, pr_number, pr_url, at, snippet) =
            row.map_err(bad_syntax)?;
        let state = match (&agent, state) {
            (Some(_), Some(s)) => Some(parse_name::<TaskState>("tasks.state", &s)?),
            _ => None,
        };
        let branch = agent
            .as_ref()
            .and(task.as_ref())
            .map(|name| format!("{TASK_BRANCH_PREFIX}{name}"));
        hits.push(HistoryHit {
            kind: parse_name("history_fts.kind", &kind)?,
            conversation_id,
            title,
            task,
            agent,
            state,
            branch,
            pr_number,
            pr_url,
            at,
            snippet: snippet.split_whitespace().collect::<Vec<_>>().join(" "),
        });
    }
    Ok(hits)
}

impl Store {
    pub async fn search_history(&self, query: HistoryQuery) -> Result<Vec<HistoryHit>> {
        self.call(move |conn| search(conn, &query)).await
    }
}
