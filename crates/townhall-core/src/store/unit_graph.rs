use std::collections::BTreeMap;

use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::codec::name_of;
use super::migrate::Migration;
use super::tasks::{find, select};
use super::Store;
use crate::logs::normalize;
use crate::model::{Task, TaskState, TASK_BRANCH_PREFIX};
use crate::{clock, Result};

pub const MIGRATIONS: &[Migration] = &[Migration {
    id: 1050,
    name: "1050_history_graph",
    sql: include_str!("migrations/1050_history_graph.sql"),
}];

const API_ERROR: &str = "api-error";
const MAX_FILES: usize = 10;
const MAX_FINGERPRINTS: usize = 5;
const MAX_REASONS: usize = 3;
const MAX_AREAS: usize = 5;
const REASON_CHARS: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Conversation,
    Task,
    Agent,
    Branch,
    Pr,
    File,
    Fingerprint,
    Outcome,
}

impl NodeKind {
    pub fn name(self) -> String {
        name_of(&self)
    }

    const INFERRED: [NodeKind; 8] = [
        NodeKind::Task,
        NodeKind::Conversation,
        NodeKind::Branch,
        NodeKind::Pr,
        NodeKind::Fingerprint,
        NodeKind::Agent,
        NodeKind::Outcome,
        NodeKind::File,
    ];

    fn tasks_sql(self) -> &'static str {
        match self {
            Self::Task => "select name from tasks where name = ?1",
            Self::Conversation => "select name from tasks where conversation_id = ?1",
            Self::Pr => {
                "select name from tasks where pr_number is not null
                 and (lower(repo || '#' || pr_number) = lower(?1) or cast(pr_number as text) = ?1)"
            }
            Self::File => {
                "select src from graph_edges where src_kind = 'task' and dst_kind = 'file'
                 and (dst glob ?1 or dst glob '*/' || ?1)"
            }
            Self::Branch => {
                "select src from graph_edges where src_kind = 'task' and dst_kind = 'branch' and dst = ?1"
            }
            Self::Agent => {
                "select src from graph_edges where src_kind = 'task' and dst_kind = 'agent' and dst = ?1"
            }
            Self::Fingerprint => {
                "select src from graph_edges where src_kind = 'task' and dst_kind = 'fingerprint' and dst = ?1"
            }
            Self::Outcome => {
                "select src from graph_edges where src_kind = 'task' and dst_kind = 'outcome' and dst = ?1"
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphQuery {
    pub kind: Option<NodeKind>,
    pub node: String,
    pub since_ms: Option<i64>,
    pub limit: usize,
    pub deep: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GraphTask {
    pub name: String,
    pub title: String,
    pub conversation: String,
    pub conversation_title: String,
    pub agent: String,
    pub outcome: Option<String>,
    pub branch: String,
    pub pr: Option<String>,
    pub at: String,
    pub retries: u32,
    pub handbacks: u64,
    pub files: Vec<String>,
    pub fingerprints: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GraphAnswer {
    pub kind: NodeKind,
    pub node: String,
    pub matched: u64,
    pub tasks: Vec<GraphTask>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AgentStat {
    pub agent: String,
    pub tasks: u64,
    pub outcomes: BTreeMap<String, u64>,
    pub avg_retries: f64,
    pub handbacks: u64,
    pub failures: Vec<(String, u64)>,
    pub areas: Vec<(String, u64)>,
}

fn outcome_of(state: TaskState) -> Option<&'static str> {
    match state {
        TaskState::Merged => Some("merged"),
        TaskState::Error => Some("failed"),
        TaskState::NeedsYou => Some("needs-user"),
        TaskState::Ended => Some("ended"),
        TaskState::InCi | TaskState::InReview => Some("in-review"),
        TaskState::Queued | TaskState::Working => None,
    }
}

fn edge(
    conn: &Connection,
    (src_kind, src): (&str, &str),
    rel: &str,
    (dst_kind, dst): (&str, &str),
) -> Result<()> {
    conn.execute(
        "insert or ignore into graph_edges (src_kind, src, rel, dst_kind, dst, at)
         values (?1, ?2, ?3, ?4, ?5, ?6)",
        params![src_kind, src, rel, dst_kind, dst, clock::now_rfc3339()],
    )?;
    Ok(())
}

fn current_outcome(conn: &Connection, task: &str) -> Result<Option<String>> {
    Ok(conn
        .query_row(
            "select dst from graph_edges where src_kind = 'task' and src = ?1 and dst_kind = 'outcome'",
            [task],
            |r| r.get(0),
        )
        .optional()?)
}

pub(super) fn sync_task(conn: &Connection, task: &Task) -> Result<()> {
    let branch = task.branch();
    edge(
        conn,
        ("conversation", &task.conversation_id),
        "has",
        ("task", &task.name),
    )?;
    edge(conn, ("task", &task.name), "ran_as", ("agent", &task.agent))?;
    edge(conn, ("task", &task.name), "on", ("branch", &branch))?;
    if let Some(pr) = &task.pr {
        let id = format!("{}#{}", task.repo, pr.number);
        edge(conn, ("branch", &branch), "opened", ("pr", &id))?;
    }
    if matches!(task.state, TaskState::Queued | TaskState::Working) {
        conn.execute("delete from graph_recorded where task = ?1", [&task.name])?;
    }
    let kept = current_outcome(conn, &task.name)?.filter(|o| o == API_ERROR);
    let outcome = match (task.state, kept) {
        (TaskState::Error, Some(kept)) => Some(kept),
        (state, _) => outcome_of(state).map(String::from),
    };
    conn.execute(
        "delete from graph_edges where src_kind = 'task' and src = ?1 and dst_kind = 'outcome'",
        [&task.name],
    )?;
    if let Some(outcome) = outcome {
        edge(
            conn,
            ("task", &task.name),
            "ended_as",
            ("outcome", &outcome),
        )?;
    }
    Ok(())
}

fn failure_text(conn: &Connection, task: &str) -> Result<Option<String>> {
    Ok(conn
        .query_row(
            "select substr(json_extract(data, '$.text'), length('task ' || ?1 || ' failed: ') + 1)
             from events
             where kind = 'event'
               and lower(json_extract(data, '$.text')) like lower('task ' || ?1 || ' failed: %')
             order by id desc limit 1",
            [task],
            |r| r.get(0),
        )
        .optional()?)
}

fn is_api_error(text: &str) -> bool {
    text.to_lowercase().contains("api error")
}

fn record(conn: &mut Connection, name: &str, files: &[String], prints: &[String]) -> Result<()> {
    let tx = conn.transaction()?;
    let Some(task) = find(&tx, name)? else {
        return Err(crate::Error::missing("task", name));
    };
    tx.execute(
        "delete from graph_edges where src_kind = 'task' and src = ?1
         and dst_kind in ('file', 'fingerprint')",
        [&task.name],
    )?;
    for file in files {
        edge(&tx, ("task", &task.name), "touched", ("file", file))?;
    }
    for print in prints {
        edge(&tx, ("task", &task.name), "hit", ("fingerprint", print))?;
    }
    sync_task(&tx, &task)?;
    if task.state == TaskState::Error
        && failure_text(&tx, &task.name)?.is_some_and(|t| is_api_error(&t))
    {
        tx.execute(
            "update graph_edges set dst = ?2
             where src_kind = 'task' and src = ?1 and dst_kind = 'outcome'",
            params![task.name, API_ERROR],
        )?;
    }
    tx.execute(
        "insert or replace into graph_recorded (task, at) values (?1, ?2)",
        params![task.name, clock::now_rfc3339()],
    )?;
    tx.commit()?;
    Ok(())
}

fn unrecorded(conn: &Connection) -> Result<Vec<Task>> {
    select(
        conn,
        "where state in ('ended', 'merged', 'error')
         and name not in (select task from graph_recorded)",
        &[],
    )
}

fn names_of(conn: &Connection, kind: NodeKind, node: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(kind.tasks_sql())?;
    let rows = stmt.query_map([node], |r| r.get::<_, String>(0))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn node_id(kind: NodeKind, node: &str) -> String {
    match kind {
        NodeKind::Branch if !node.starts_with(TASK_BRANCH_PREFIX) => {
            format!("{TASK_BRANCH_PREFIX}{node}")
        }
        _ => node.to_string(),
    }
}

fn resolve(conn: &Connection, query: &GraphQuery) -> Result<Option<(NodeKind, String)>> {
    let node = query.node.trim();
    let kinds: Vec<NodeKind> = query
        .kind
        .map_or_else(|| NodeKind::INFERRED.to_vec(), |k| vec![k]);
    for kind in kinds {
        let id = node_id(kind, node);
        if !names_of(conn, kind, &id)?.is_empty() {
            return Ok(Some((kind, id)));
        }
    }
    Ok(None)
}

fn strings(conn: &Connection, sql: &str, name: &str, max: usize) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params![name, max], |r| r.get::<_, String>(0))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn graph(conn: &Connection, query: &GraphQuery) -> Result<Option<GraphAnswer>> {
    let Some((kind, node)) = resolve(conn, query)? else {
        return Ok(None);
    };
    let mut params = vec![Value::Text(node.clone())];
    let mut since = String::new();
    if let Some(ms) = query.since_ms {
        params.push(Value::Text(clock::from_unix_millis(ms)));
        since = " and t.created_at >= ?2".into();
    }
    let matched: u64 = conn.query_row(
        &format!(
            "select count(*) from tasks t where t.name in ({}){since}",
            kind.tasks_sql()
        ),
        params_from_iter(params.clone()),
        |r| r.get(0),
    )?;
    params.push(Value::Integer(
        i64::try_from(query.limit).unwrap_or(i64::MAX),
    ));
    let sql = format!(
        "select t.name, t.title, t.conversation_id, c.title, t.agent, t.repo, t.pr_number,
                t.created_at, o.dst, coalesce(json_extract(t.verify, '$.retries'), 0),
                (select count(*) from task_handoffs h where h.task = t.name)
         from tasks t
         join conversations c on c.id = t.conversation_id
         left join graph_edges o
           on o.src_kind = 'task' and o.src = t.name and o.dst_kind = 'outcome'
         where t.name in ({}){since}
         order by t.created_at desc, t.name desc
         limit ?{}",
        kind.tasks_sql(),
        params.len()
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut tasks = stmt
        .query_map(params_from_iter(params), |r| {
            let repo: String = r.get(5)?;
            let number: Option<u64> = r.get(6)?;
            let name: String = r.get(0)?;
            Ok(GraphTask {
                branch: format!("{TASK_BRANCH_PREFIX}{name}"),
                name,
                title: r.get(1)?,
                conversation: r.get(2)?,
                conversation_title: r.get(3)?,
                agent: r.get(4)?,
                pr: number.map(|n| format!("{repo}#{n}")),
                at: r.get(7)?,
                outcome: r.get(8)?,
                retries: r.get(9)?,
                handbacks: r.get(10)?,
                files: vec![],
                fingerprints: vec![],
            })
        })?
        .collect::<rusqlite::Result<Vec<GraphTask>>>()?;
    if query.deep || kind == NodeKind::Task {
        for task in &mut tasks {
            task.files = strings(
                conn,
                "select dst from graph_edges where src_kind = 'task' and src = ?1
                 and dst_kind = 'file' order by dst limit ?2",
                &task.name,
                MAX_FILES,
            )?;
            task.fingerprints = strings(
                conn,
                "select dst from graph_edges where src_kind = 'task' and src = ?1
                 and dst_kind = 'fingerprint' order by dst limit ?2",
                &task.name,
                MAX_FINGERPRINTS,
            )?;
        }
    }
    Ok(Some(GraphAnswer {
        kind,
        node,
        matched,
        tasks,
    }))
}

fn area(path: &str) -> String {
    match path.rsplit_once('/') {
        Some((dir, _)) => dir.to_string(),
        None => ".".to_string(),
    }
}

fn reason(text: &str, task: &str) -> String {
    let text = normalize(text.trim(), &[("task", task)]);
    text.chars().take(REASON_CHARS).collect()
}

fn ranked(counts: BTreeMap<String, u64>, max: usize) -> Vec<(String, u64)> {
    let mut list: Vec<(String, u64)> = counts.into_iter().collect();
    list.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    list.truncate(max);
    list
}

fn stats(conn: &Connection, since_ms: Option<i64>) -> Result<Vec<AgentStat>> {
    let since = since_ms.map_or_else(String::new, clock::from_unix_millis);
    let mut stmt = conn.prepare(
        "select t.name, t.agent, o.dst, coalesce(json_extract(t.verify, '$.retries'), 0)
         from tasks t
         left join graph_edges o
           on o.src_kind = 'task' and o.src = t.name and o.dst_kind = 'outcome'
         where t.created_at >= ?1
         order by t.agent collate nocase, t.created_at",
    )?;
    let rows = stmt
        .query_map([&since], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, u64>(3)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut by_agent: BTreeMap<String, AgentStat> = BTreeMap::new();
    let mut reasons: BTreeMap<String, BTreeMap<String, u64>> = BTreeMap::new();
    let mut areas: BTreeMap<String, BTreeMap<String, u64>> = BTreeMap::new();
    let mut retries: BTreeMap<String, u64> = BTreeMap::new();
    for (name, agent, outcome, tried) in rows {
        let stat = by_agent.entry(agent.clone()).or_insert_with(|| AgentStat {
            agent: agent.clone(),
            tasks: 0,
            outcomes: BTreeMap::new(),
            avg_retries: 0.0,
            handbacks: 0,
            failures: vec![],
            areas: vec![],
        });
        stat.tasks += 1;
        *retries.entry(agent.clone()).or_default() += tried;
        let outcome = outcome.unwrap_or_else(|| "open".into());
        *stat.outcomes.entry(outcome.clone()).or_default() += 1;
        if !matches!(outcome.as_str(), "failed" | API_ERROR | "needs-user") {
            continue;
        }
        if outcome != "needs-user" {
            let text = failure_text(conn, &name)?.unwrap_or_else(|| "unknown".into());
            *reasons
                .entry(agent.clone())
                .or_default()
                .entry(reason(&text, &name))
                .or_default() += 1;
        }
        for file in strings(
            conn,
            "select dst from graph_edges where src_kind = 'task' and src = ?1
             and dst_kind = 'file' order by dst limit ?2",
            &name,
            usize::MAX >> 1,
        )? {
            *areas
                .entry(agent.clone())
                .or_default()
                .entry(area(&file))
                .or_default() += 1;
        }
    }
    let mut handed = conn.prepare(
        "select from_type, count(*) from task_handoffs where at >= ?1 group by from_type",
    )?;
    let handed: Vec<(String, u64)> = handed
        .query_map([&since], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    for (agent, count) in handed {
        if let Some(stat) = by_agent
            .values_mut()
            .find(|s| s.agent.eq_ignore_ascii_case(&agent))
        {
            stat.handbacks = count;
        }
    }
    Ok(by_agent
        .into_values()
        .map(|mut stat| {
            stat.avg_retries = retries.remove(&stat.agent).unwrap_or(0) as f64 / stat.tasks as f64;
            stat.failures = ranked(reasons.remove(&stat.agent).unwrap_or_default(), MAX_REASONS);
            stat.areas = ranked(areas.remove(&stat.agent).unwrap_or_default(), MAX_AREAS);
            stat
        })
        .collect())
}

impl Store {
    pub async fn history_graph(&self, query: GraphQuery) -> Result<Option<GraphAnswer>> {
        self.call(move |conn| graph(conn, &query)).await
    }

    pub async fn record_task_graph(
        &self,
        task: &str,
        files: Vec<String>,
        fingerprints: Vec<String>,
    ) -> Result<()> {
        let task = task.to_string();
        self.call(move |conn| record(conn, &task, &files, &fingerprints))
            .await
    }

    pub async fn unrecorded_graph_tasks(&self) -> Result<Vec<Task>> {
        self.call(|conn| unrecorded(conn)).await
    }

    pub async fn agent_stats(&self, since_ms: Option<i64>) -> Result<Vec<AgentStat>> {
        self.call(move |conn| stats(conn, since_ms)).await
    }
}
