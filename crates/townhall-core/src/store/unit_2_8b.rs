//! Unit 2.8b library (agent types, 2.12): the `agents` table, one row per saved agent type
//! with its TOML as written. Every save runs 2.8a's `check_agent` first, every read parses
//! the TOML again. `0801_seed_builder` saves the type `builder` once, on the first open
//! with no agent types; it is never saved again after you delete it. `agent_type_asks`
//! keeps the agent types townhall's session asked you to confirm, one per conversation and
//! name, so a confirmed save can be checked against what you were shown. `0803_task_agent`
//! adds `tasks.agent`, the type a task runs as (`builder` for tasks from before it).
//! Migrations 0800 to 0849, files `migrations/08xx_<name>.sql`.

use rusqlite::{params, Connection, OptionalExtension};

use super::migrate::Migration;
use super::Store;
use crate::agent_usage::{aggregate, AgentUsageSeries, UsageRange, UsageRun};
use crate::definitions::{check_agent, AgentDef, DefKind, DefinitionError, Parsed};
use crate::model::{AgentType, AgentUsage};
use crate::{clock, Error, Result};

pub const MIGRATIONS: &[Migration] = &[
    Migration {
        id: 800,
        name: "0800_agents",
        sql: include_str!("migrations/0800_agents.sql"),
    },
    Migration {
        id: 801,
        name: "0801_seed_builder",
        sql: include_str!("migrations/0801_seed_builder.sql"),
    },
    Migration {
        id: 802,
        name: "0802_agent_type_asks",
        sql: include_str!("migrations/0802_agent_type_asks.sql"),
    },
    Migration {
        id: 803,
        name: "0803_task_agent",
        sql: include_str!("migrations/0803_task_agent.sql"),
    },
    Migration {
        id: 804,
        name: "0804_task_tokens",
        sql: include_str!("migrations/0804_task_tokens.sql"),
    },
];

/// A stored row: name, source, updated_at.
type Row = (String, String, String);

fn rows(conn: &Connection, filter: &str, name: Option<&str>) -> Result<Vec<Row>> {
    let sql = format!(
        "select name, source, updated_at from agents {filter} order by name collate nocase"
    );
    let mut stmt = conn.prepare(&sql)?;
    let map =
        |r: &rusqlite::Row<'_>| -> rusqlite::Result<Row> { Ok((r.get(0)?, r.get(1)?, r.get(2)?)) };
    let found: rusqlite::Result<Vec<Row>> = match name {
        Some(name) => stmt.query_map([name], map)?.collect(),
        None => stmt.query_map([], map)?.collect(),
    };
    Ok(found?)
}

/// The row parsed again. A row that no longer passes the checks is corrupt.
fn parse_row((name, source, updated_at): Row) -> Result<AgentType> {
    let parsed = check_agent(&source).map_err(|errors| Error::Corrupt {
        path: format!("townhall.db agents.source of `{name}`"),
        message: joined(&errors),
    })?;
    Ok(AgentType {
        def: parsed.def,
        source: parsed.source,
        updated_at,
    })
}

/// Every error on one line, without the `error: ` each starts with.
pub(crate) fn joined(errors: &[DefinitionError]) -> String {
    errors
        .iter()
        .map(|e| {
            let text = e.to_string();
            text.strip_prefix("error: ").unwrap_or(&text).to_string()
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn too_long(source: &str) -> Option<DefinitionError> {
    let chars = source.chars().count();
    (chars > Store::MAX_AGENT_SOURCE).then(|| DefinitionError {
        kind: DefKind::Agent,
        name: None,
        step: None,
        field: None,
        message: format!(
            "the toml is {chars} characters, over the limit of {}: a type's prompt goes on claude's command line",
            Store::MAX_AGENT_SOURCE
        ),
        line: None,
    })
}

fn upsert(conn: &mut Connection, def: &AgentDef, source: &str) -> Result<(AgentType, bool)> {
    let tx = conn.transaction()?;
    let replaced = tx
        .query_row("select 1 from agents where name = ?1", [&def.name], |r| {
            r.get::<_, i64>(0)
        })
        .optional()?
        .is_some();
    let updated_at = clock::now_rfc3339();
    tx.execute(
        "insert into agents (name, description, source, updated_at) values (?1, ?2, ?3, ?4)
         on conflict (name) do update set name = excluded.name,
         description = excluded.description, source = excluded.source,
         updated_at = excluded.updated_at",
        params![def.name, def.description, source, updated_at],
    )?;
    tx.commit()?;
    let saved = AgentType {
        def: def.clone(),
        source: source.to_string(),
        updated_at,
    };
    Ok((saved, replaced))
}

fn delete(conn: &mut Connection, name: &str) -> Result<String> {
    let tx = conn.transaction()?;
    let stored: String = tx
        .query_row("select name from agents where name = ?1", [name], |r| {
            r.get(0)
        })
        .optional()?
        .ok_or_else(|| Error::missing("agent type", name))?;
    let count: i64 = tx.query_row("select count(*) from agents", [], |r| r.get(0))?;
    if count <= 1 {
        return Err(Error::Conflict(format!(
            "agent type `{stored}` is the last one: every task needs a type to run, save another type first"
        )));
    }
    tx.execute("delete from agents where name = ?1", [&stored])?;
    tx.commit()?;
    Ok(stored)
}

impl Store {
    /// The longest agent type TOML, in characters. Its prompt goes on claude's command
    /// line, which tmux takes as one argument of about 16 k at most.
    pub const MAX_AGENT_SOURCE: usize = 10_000;

    /// What every save and import runs: the length limit, then 2.8a's `check_agent`.
    pub fn check_agent_type(
        source: &str,
    ) -> std::result::Result<Parsed<AgentDef>, Vec<DefinitionError>> {
        match too_long(source) {
            Some(error) => Err(vec![error]),
            None => check_agent(source),
        }
    }

    /// Every saved agent type by name, ignoring case. A row that no longer parses is left
    /// out with a warning, so one bad row cannot hide the rest.
    pub async fn agent_types(&self) -> Result<Vec<AgentType>> {
        let found = self.call(|conn| rows(conn, "", None)).await?;
        Ok(found
            .into_iter()
            .filter_map(|row| match parse_row(row) {
                Ok(agent) => Some(agent),
                Err(e) => {
                    tracing::warn!(
                        target: "townhall::store", event = "store.agent_type_unreadable",
                        error_kind = e.kind(), error = %crate::redact::redact(&e.to_string()),
                        "agent type row skipped"
                    );
                    None
                }
            })
            .collect())
    }

    /// The agent type called `name`, ignoring case.
    pub async fn agent_type(&self, name: &str) -> Result<Option<AgentType>> {
        let name = name.to_string();
        let found = self
            .call(move |conn| rows(conn, "where name = ?1", Some(&name)))
            .await?;
        found.into_iter().next().map(parse_row).transpose()
    }

    /// Checks `source` and saves it under its `name`, replacing a type of that name
    /// (ignoring case, the new spelling wins). `true` when it replaced one.
    pub async fn save_agent_type(&self, source: &str) -> Result<(AgentType, bool)> {
        let parsed = Self::check_agent_type(source)
            .map_err(|errors| Error::InvalidInput(joined(&errors)))?;
        self.call(move |conn| upsert(conn, &parsed.def, &parsed.source))
            .await
    }

    /// Deletes the type called `name` (ignoring case) and returns its stored name.
    /// `Missing` when unknown, `Conflict` for the last type: tasks always need one.
    pub async fn delete_agent_type(&self, name: &str) -> Result<String> {
        let name = name.to_string();
        self.call(move |conn| delete(conn, &name)).await
    }
}

impl Store {
    pub async fn set_task_tokens(&self, task: &str, tokens: u64) -> Result<()> {
        let (name, tokens) = (task.to_string(), i64::try_from(tokens).unwrap_or(i64::MAX));
        self.call(move |conn| {
            let changed = conn.execute(
                "update tasks set tokens = ?2 where name = ?1",
                params![name, tokens],
            )?;
            match changed {
                0 => Err(Error::missing("task", &name)),
                _ => Ok(()),
            }
        })
        .await
    }

    /// Runs and tokens of the agent type `agent` (ignoring case) over `range`, ending at
    /// `now_ms`, with the top tasks, repos and conversations. Echoes `agent` as saved when
    /// the type exists.
    pub async fn agent_usage_series(
        &self,
        agent: &str,
        range: UsageRange,
        now_ms: i64,
    ) -> Result<AgentUsageSeries> {
        let name = match self.agent_type(agent).await? {
            Some(saved) => saved.def.name,
            None => agent.to_string(),
        };
        let wanted = name.clone();
        let runs = self
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "select name, repo, conversation_id, created_at, tokens from tasks
                     where agent = ?1 collate nocase",
                )?;
                let rows = stmt
                    .query_map([wanted], |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, String>(1)?,
                            r.get::<_, String>(2)?,
                            r.get::<_, String>(3)?,
                            r.get::<_, i64>(4)?,
                        ))
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                rows.into_iter()
                    .map(|(name, repo, conversation_id, created_at, tokens)| {
                        let at_ms = clock::parse_rfc3339(&created_at).ok_or_else(|| {
                            Error::Database(format!(
                                "task `{name}` has an unreadable created_at `{created_at}`"
                            ))
                        })?;
                        Ok(UsageRun {
                            name,
                            repo,
                            conversation_id,
                            at_ms,
                            tokens: u64::try_from(tokens).unwrap_or(0),
                        })
                    })
                    .collect::<Result<Vec<_>>>()
            })
            .await?;
        Ok(aggregate(&name, range, now_ms, &runs))
    }

    pub async fn agent_usage(&self) -> Result<Vec<AgentUsage>> {
        self.call(|conn| {
            let mut stmt = conn.prepare(
                "select agent, count(*), sum(tokens) from tasks
                 group by agent order by agent collate nocase",
            )?;
            let rows = stmt
                .query_map([], |r| {
                    Ok(AgentUsage {
                        agent: r.get(0)?,
                        runs: r.get(1)?,
                        tokens: r.get(2)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })
        .await
    }
}

impl Store {
    /// Records that townhall showed `source` for the type `name` in the entry `entry_id`
    /// of the conversation and asked you to confirm. Replaces an earlier ask for that name.
    pub async fn ask_agent_type(
        &self,
        conversation_id: &str,
        name: &str,
        source: &str,
        entry_id: u64,
    ) -> Result<()> {
        let (cid, name, source) = (
            conversation_id.to_string(),
            name.to_string(),
            source.to_string(),
        );
        self.call(move |conn| {
            conn.execute(
                "insert or replace into agent_type_asks (conversation_id, name, source, entry_id, asked_at)
                 values (?1, ?2, ?3, ?4, ?5)",
                params![cid, name, source, entry_id, clock::now_rfc3339()],
            )?;
            Ok(())
        })
        .await
    }

    /// The source and entry id of the conversation's open ask for the type `name`.
    pub async fn agent_type_ask(
        &self,
        conversation_id: &str,
        name: &str,
    ) -> Result<Option<(String, u64)>> {
        let (cid, name) = (conversation_id.to_string(), name.to_string());
        self.call(move |conn| {
            Ok(conn
                .query_row(
                    "select source, entry_id from agent_type_asks where conversation_id = ?1 and name = ?2",
                    params![cid, name],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?)
        })
        .await
    }

    /// Closes the conversation's ask for the type `name`, once it was answered.
    pub async fn forget_agent_type_ask(&self, conversation_id: &str, name: &str) -> Result<()> {
        let (cid, name) = (conversation_id.to_string(), name.to_string());
        self.call(move |conn| {
            conn.execute(
                "delete from agent_type_asks where conversation_id = ?1 and name = ?2",
                params![cid, name],
            )?;
            Ok(())
        })
        .await
    }
}
