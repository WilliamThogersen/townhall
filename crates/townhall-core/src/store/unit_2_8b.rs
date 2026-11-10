//! Unit 2.8b library (agent types, 2.12): the `agents` table, one row per saved agent type
//! with its TOML as written. Every save runs 2.8a's `check_agent` first, every read parses
//! the TOML again. `0801_seed_builder` saves the type `builder` once, on the first open
//! with no agent types; it is never saved again after you delete it. `agent_type_asks`
//! keeps the agent types townhall's session asked you to confirm, one per conversation and
//! name, so a confirmed save can be checked against what you were shown. `0803_task_agent`
//! adds `tasks.agent`, the type a task runs as (`builder` for tasks from before it).
//! `0806_seed_default` saves `default`, the type townhall's own conversations run as, when
//! none of that name exists; it cannot be deleted and is not a type tasks default to.
//! Migrations 0800 to 0849, files `migrations/08xx_<name>.sql`.

use rusqlite::{params, Connection, OptionalExtension};

use super::migrate::Migration;
use super::Store;
use crate::agent_usage::{aggregate, AgentUsageSeries, UsageRange, UsageRun};
use crate::definitions::{check_agent, AgentDef, DefKind, DefinitionError, Parsed};
use crate::model::{AgentType, AgentUsage, Origin, TaskRun, CONVERSATION_AGENT_TYPE};
use crate::plugins::agent_types;
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
    Migration {
        id: 805,
        name: "0805_task_model",
        sql: include_str!("migrations/0805_task_model.sql"),
    },
    Migration {
        id: 806,
        name: "0806_seed_default",
        sql: include_str!("migrations/0806_seed_default.sql"),
    },
    Migration {
        id: 807,
        name: "0807_default_model",
        sql: include_str!("migrations/0807_default_model.sql"),
    },
    Migration {
        id: 808,
        name: "0808_default_web_tools",
        sql: include_str!("migrations/0808_default_web_tools.sql"),
    },
    Migration {
        id: 809,
        name: "0809_conversation_turns",
        sql: include_str!("migrations/0809_conversation_turns.sql"),
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
        origin: Origin::User,
        plugin: None,
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
        origin: Origin::User,
        plugin: None,
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
    if stored.eq_ignore_ascii_case(CONVERSATION_AGENT_TYPE) {
        return Err(Error::Conflict(format!(
            "agent type `{stored}` runs the conversations and cannot be deleted: edit it instead"
        )));
    }
    let count: i64 = tx.query_row(
        "select count(*) from agents where name <> ?1 collate nocase",
        [CONVERSATION_AGENT_TYPE],
        |r| r.get(0),
    )?;
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

    /// Every agent type by name, ignoring case: the saved ones, then the read only types
    /// of enabled plugins that no saved type shadows. A row that no longer parses is left
    /// out with a warning, so one bad row cannot hide the rest.
    pub async fn agent_types(&self) -> Result<Vec<AgentType>> {
        let found = self.call(|conn| rows(conn, "", None)).await?;
        let mut all: Vec<AgentType> = found
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
            .collect();
        for plugin_type in agent_types::enabled_types(self).await? {
            let taken = all
                .iter()
                .any(|t| t.def.name.eq_ignore_ascii_case(&plugin_type.def.name));
            if !taken {
                all.push(plugin_type);
            }
        }
        all.sort_by_key(|t| t.def.name.to_lowercase());
        Ok(all)
    }

    /// The agent type called `name`, ignoring case.
    pub async fn agent_type(&self, name: &str) -> Result<Option<AgentType>> {
        if let Some(saved) = self.saved_agent_type(name).await? {
            return Ok(Some(saved));
        }
        Ok(agent_types::enabled_types(self)
            .await?
            .into_iter()
            .find(|t| t.def.name.eq_ignore_ascii_case(name)))
    }

    async fn saved_agent_type(&self, name: &str) -> Result<Option<AgentType>> {
        let name = name.to_string();
        let found = self
            .call(move |conn| rows(conn, "where name = ?1", Some(&name)))
            .await?;
        found.into_iter().next().map(parse_row).transpose()
    }

    async fn refuse_plugin_type(&self, name: &str) -> Result<()> {
        if self.saved_agent_type(name).await?.is_some() {
            return Ok(());
        }
        match self.agent_type(name).await? {
            Some(AgentType {
                plugin: Some(plugin),
                def,
                ..
            }) => Err(Error::Conflict(format!(
                "agent type `{}` is owned by plugin {plugin} and read only: copy it to a type of your own",
                def.name
            ))),
            _ => Ok(()),
        }
    }

    /// Checks `source` and saves it under its `name`, replacing a type of that name
    /// (ignoring case, the new spelling wins). `true` when it replaced one. `Conflict`
    /// for the name of a plugin's type.
    pub async fn save_agent_type(&self, source: &str) -> Result<(AgentType, bool)> {
        let parsed = Self::check_agent_type(source)
            .map_err(|errors| Error::InvalidInput(joined(&errors)))?;
        self.refuse_plugin_type(&parsed.def.name).await?;
        self.call(move |conn| upsert(conn, &parsed.def, &parsed.source))
            .await
    }

    /// Deletes the type called `name` (ignoring case) and returns its stored name.
    /// `Missing` when unknown, `Conflict` for the last type: tasks always need one, and
    /// for a plugin's type.
    pub async fn delete_agent_type(&self, name: &str) -> Result<String> {
        self.refuse_plugin_type(name).await?;
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
                     where agent = ?1 collate nocase
                     union all
                     select c.title, coalesce(c.linked_repo, ''), t.conversation_id,
                            t.created_at, t.tokens
                     from conversation_turns t join conversations c on c.id = t.conversation_id
                     where ?1 = ?2 collate nocase",
                )?;
                let rows = stmt
                    .query_map(params![wanted, CONVERSATION_AGENT_TYPE], |r| {
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

    pub async fn record_turn(
        &self,
        conversation_id: &str,
        state: &str,
        tokens: u64,
        created_at: &str,
        ended_at: &str,
    ) -> Result<()> {
        let id = conversation_id.to_string();
        let (state, created_at, ended_at) = (
            state.to_string(),
            created_at.to_string(),
            ended_at.to_string(),
        );
        let tokens = i64::try_from(tokens).unwrap_or(i64::MAX);
        self.call(move |conn| {
            let known = conn
                .query_row("select 1 from conversations where id = ?1", [&id], |_| {
                    Ok(())
                })
                .optional()?;
            if known.is_none() {
                return Err(Error::missing("conversation", &id));
            }
            conn.execute(
                "insert into conversation_turns (conversation_id, state, tokens, created_at, ended_at)
                 values (?1, ?2, ?3, ?4, ?5)",
                params![id, state, tokens, created_at, ended_at],
            )?;
            Ok(())
        })
        .await
    }

    pub async fn set_task_model(&self, task: &str, model: &str) -> Result<()> {
        let (name, model) = (task.to_string(), model.to_string());
        self.call(move |conn| {
            let changed = conn.execute(
                "update tasks set model = ?2 where name = ?1",
                params![name, model],
            )?;
            match changed {
                0 => Err(Error::missing("task", &name)),
                _ => Ok(()),
            }
        })
        .await
    }

    pub async fn agent_runs(&self, agent: &str, limit: u32) -> Result<Vec<TaskRun>> {
        let agent = agent.to_string();
        self.call(move |conn| {
            let mut stmt = conn.prepare(
                "select 'task', conversation_id, name, repo, state, model, tokens, created_at,
                        ended_at
                 from tasks where agent = ?1 collate nocase
                 union all
                 select 'turn', t.conversation_id, c.title, coalesce(c.linked_repo, ''), t.state,
                        null, t.tokens, t.created_at, t.ended_at
                 from conversation_turns t join conversations c on c.id = t.conversation_id
                 where ?1 = ?3 collate nocase
                 order by 8 desc, 3 limit ?2",
            )?;
            let rows = stmt
                .query_map(params![agent, limit, CONVERSATION_AGENT_TYPE], |r| {
                    Ok(TaskRun {
                        kind: r.get(0)?,
                        conversation_id: r.get(1)?,
                        task: r.get(2)?,
                        repo: r.get(3)?,
                        state: r.get(4)?,
                        model: r.get(5)?,
                        tokens: r.get(6)?,
                        created_at: r.get(7)?,
                        ended_at: r.get(8)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })
        .await
    }

    pub async fn agent_usage(&self) -> Result<Vec<AgentUsage>> {
        self.call(|conn| {
            let mut stmt = conn.prepare(
                "select agent, count(*), sum(tokens) from (
                   select agent, tokens from tasks
                   union all
                   select ?1, tokens from conversation_turns
                 ) group by agent order by agent collate nocase",
            )?;
            let rows = stmt
                .query_map([CONVERSATION_AGENT_TYPE], |r| {
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
