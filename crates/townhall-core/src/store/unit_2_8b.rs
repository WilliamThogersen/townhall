//! Unit 2.8b library (agent types, 2.12): the `agents` table, one row per saved agent type
//! with its TOML as written. Every save runs 2.8a's `check_agent` first, every read parses
//! the TOML again. Migrations 0800 to 0849, files `migrations/08xx_<name>.sql`.

use rusqlite::{params, Connection, OptionalExtension};

use super::migrate::Migration;
use super::Store;
use crate::definitions::{check_agent, AgentDef, DefKind, DefinitionError, Parsed};
use crate::model::AgentType;
use crate::{clock, Error, Result};

pub const MIGRATIONS: &[Migration] = &[Migration {
    id: 800,
    name: "0800_agents",
    sql: include_str!("migrations/0800_agents.sql"),
}];

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
                    eprintln!("warn: {e}");
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
