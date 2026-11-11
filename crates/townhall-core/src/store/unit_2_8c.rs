//! Unit 2.8c pipelines and runs: the tables it owns and their store functions (`impl Store`
//! blocks here, using `Store::call`). Migrations 0850 to 0899, files `migrations/08xx_<name>.sql`.
//! The user-facing name is pipeline, the code's is workflow.
//! `0850_workflows` adds `workflows` (one row per saved pipeline with its TOML as written),
//! `workflow_asks` (the pipelines townhall's session asked you to confirm, like
//! `agent_type_asks`), and `runs` and `run_steps` (no store functions yet).
//! `0851_default_pipeline_tools` adds the three pipeline tools to the stored `default` type.

use rusqlite::{params, Connection, OptionalExtension};

use super::migrate::Migration;
use super::unit_2_8b::joined;
use super::Store;
use crate::definitions::{check_pipeline, DefinitionError, KnownAgent, Parsed, WorkflowDef};
use crate::model::Workflow;
use crate::{clock, Error, Result};

pub const MIGRATIONS: &[Migration] = &[
    Migration {
        id: 850,
        name: "0850_workflows",
        sql: include_str!("migrations/0850_workflows.sql"),
    },
    Migration {
        id: 851,
        name: "0851_default_pipeline_tools",
        sql: include_str!("migrations/0851_default_pipeline_tools.sql"),
    },
];

type Row = (String, String, String);

fn rows(conn: &Connection, filter: &str, name: Option<&str>) -> Result<Vec<Row>> {
    let sql = format!(
        "select name, source, updated_at from workflows {filter} order by name collate nocase"
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

/// The row parsed again. Agents are not checked again: deleting one later must not hide a
/// pipeline that names it.
fn parse_row((name, source, updated_at): Row) -> Result<Workflow> {
    let parsed = crate::definitions::parse_workflow(&source).map_err(|e| Error::Corrupt {
        path: format!("townhall.db workflows.source of `{name}`"),
        message: joined(&[e]),
    })?;
    Ok(Workflow {
        def: parsed.def,
        source: parsed.source,
        updated_at,
    })
}

fn upsert(conn: &mut Connection, def: &WorkflowDef, source: &str) -> Result<(Workflow, bool)> {
    let tx = conn.transaction()?;
    let replaced = tx
        .query_row("select 1 from workflows where name = ?1", [&def.name], |r| {
            r.get::<_, i64>(0)
        })
        .optional()?
        .is_some();
    let updated_at = clock::now_rfc3339();
    tx.execute(
        "insert into workflows (name, description, source, updated_at) values (?1, ?2, ?3, ?4)
         on conflict (name) do update set name = excluded.name,
         description = excluded.description, source = excluded.source,
         updated_at = excluded.updated_at",
        params![def.name, def.description, source, updated_at],
    )?;
    tx.commit()?;
    let saved = Workflow {
        def: def.clone(),
        source: source.to_string(),
        updated_at,
    };
    Ok((saved, replaced))
}

fn delete(conn: &mut Connection, name: &str) -> Result<String> {
    let tx = conn.transaction()?;
    let stored: String = tx
        .query_row("select name from workflows where name = ?1", [name], |r| {
            r.get(0)
        })
        .optional()?
        .ok_or_else(|| Error::missing("pipeline", name))?;
    tx.execute("delete from workflows where name = ?1", [&stored])?;
    tx.commit()?;
    Ok(stored)
}

impl Store {
    /// The longest pipeline TOML, in characters.
    pub const MAX_WORKFLOW_SOURCE: usize = 50_000;

    /// Parses `source` and checks it against the saved agent types: what every save and
    /// import runs. Returns every error found.
    pub async fn check_pipeline(
        &self,
        source: &str,
    ) -> std::result::Result<Parsed<WorkflowDef>, Vec<DefinitionError>> {
        let chars = source.chars().count();
        if chars > Self::MAX_WORKFLOW_SOURCE {
            return Err(vec![DefinitionError {
                kind: crate::definitions::DefKind::Workflow,
                name: None,
                step: None,
                field: None,
                message: format!(
                    "the toml is {chars} characters, over the limit of {}",
                    Self::MAX_WORKFLOW_SOURCE
                ),
                line: None,
            }]);
        }
        let types = self
            .agent_types()
            .await
            .map_err(|e| vec![unreadable_agents(&e)])?;
        let agents: Vec<KnownAgent> = types
            .iter()
            .map(|t| KnownAgent::new(&t.def.name, &t.def.requires))
            .collect();
        check_pipeline(source, &agents)
    }

    /// Every saved pipeline by name, ignoring case.
    pub async fn workflows(&self) -> Result<Vec<Workflow>> {
        let found = self.call(|conn| rows(conn, "", None)).await?;
        found.into_iter().map(parse_row).collect()
    }

    /// The pipeline called `name`, ignoring case.
    pub async fn workflow(&self, name: &str) -> Result<Option<Workflow>> {
        let name = name.to_string();
        let found = self
            .call(move |conn| rows(conn, "where name = ?1", Some(&name)))
            .await?;
        found.into_iter().next().map(parse_row).transpose()
    }

    /// Checks `source` and saves it under its `name`, replacing a pipeline of that name
    /// (ignoring case, the new spelling wins). `true` when it replaced one. An unknown
    /// agent is `InvalidInput`.
    pub async fn save_workflow(&self, source: &str) -> Result<(Workflow, bool)> {
        let parsed = self
            .check_pipeline(source)
            .await
            .map_err(|errors| Error::InvalidInput(joined(&errors)))?;
        self.call(move |conn| upsert(conn, &parsed.def, &parsed.source))
            .await
    }

    /// Deletes the pipeline called `name` (ignoring case) and returns its stored name.
    /// `Missing` when unknown.
    pub async fn delete_workflow(&self, name: &str) -> Result<String> {
        let name = name.to_string();
        self.call(move |conn| delete(conn, &name)).await
    }

    /// Records that townhall showed `source` for the pipeline `name` in the entry
    /// `entry_id` of the conversation and asked you to confirm. Replaces an earlier ask.
    pub async fn ask_workflow(
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
                "insert or replace into workflow_asks (conversation_id, name, source, entry_id, asked_at)
                 values (?1, ?2, ?3, ?4, ?5)",
                params![cid, name, source, entry_id, clock::now_rfc3339()],
            )?;
            Ok(())
        })
        .await
    }

    /// The source and entry id of the conversation's open ask for the pipeline `name`.
    pub async fn workflow_ask(
        &self,
        conversation_id: &str,
        name: &str,
    ) -> Result<Option<(String, u64)>> {
        let (cid, name) = (conversation_id.to_string(), name.to_string());
        self.call(move |conn| {
            Ok(conn
                .query_row(
                    "select source, entry_id from workflow_asks where conversation_id = ?1 and name = ?2",
                    params![cid, name],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?)
        })
        .await
    }

    /// Closes the conversation's ask for the pipeline `name`, once it was answered.
    pub async fn forget_workflow_ask(&self, conversation_id: &str, name: &str) -> Result<()> {
        let (cid, name) = (conversation_id.to_string(), name.to_string());
        self.call(move |conn| {
            conn.execute(
                "delete from workflow_asks where conversation_id = ?1 and name = ?2",
                params![cid, name],
            )?;
            Ok(())
        })
        .await
    }
}

fn unreadable_agents(e: &Error) -> DefinitionError {
    DefinitionError {
        kind: crate::definitions::DefKind::Workflow,
        name: None,
        step: None,
        field: None,
        message: format!("the saved agent types could not be read: {e}"),
        line: None,
    }
}
