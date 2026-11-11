//! Unit 2.8c runs: the tables it owns and their store functions (`impl Store` blocks
//! here, using `Store::call`). Migrations 0850 to 0899, files `migrations/08xx_<name>.sql`.

use rusqlite::{params, Connection, OptionalExtension};

use super::migrate::Migration;
use super::unit_2_8b::joined;
use super::Store;
use crate::definitions::{
    check_default, check_pipeline, DefinitionError, KnownAgent, Parsed, WorkflowDef,
};
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
    Migration {
        id: 852,
        name: "0852_run_input",
        sql: include_str!("migrations/0852_run_input.sql"),
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
        .query_row(
            "select 1 from workflows where name = ?1",
            [&def.name],
            |r| r.get::<_, i64>(0),
        )
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
    pub const MAX_WORKFLOW_SOURCE: usize = 50_000;

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
        let parsed = check_pipeline(source, &agents)?;
        let others: Vec<WorkflowDef> = self
            .workflows()
            .await
            .map_err(|e| vec![unreadable_agents(&e)])?
            .into_iter()
            .map(|w| w.def)
            .collect();
        match check_default(&parsed.def, &others) {
            conflicts if conflicts.is_empty() => Ok(parsed),
            conflicts => Err(conflicts),
        }
    }

    pub async fn default_workflow(&self, repo: &str) -> Result<Option<Workflow>> {
        let all = self.workflows().await?;
        let defaults = |w: &Workflow| w.def.default;
        let named = all
            .iter()
            .filter(|w| defaults(w))
            .find(|w| w.def.repos.iter().any(|r| r.eq_ignore_ascii_case(repo)));
        let any = all
            .iter()
            .filter(|w| defaults(w))
            .find(|w| w.def.repos.is_empty());
        Ok(named.or(any).cloned())
    }

    pub async fn workflows(&self) -> Result<Vec<Workflow>> {
        let found = self.call(|conn| rows(conn, "", None)).await?;
        found.into_iter().map(parse_row).collect()
    }

    pub async fn workflow(&self, name: &str) -> Result<Option<Workflow>> {
        let name = name.to_string();
        let found = self
            .call(move |conn| rows(conn, "where name = ?1", Some(&name)))
            .await?;
        found.into_iter().next().map(parse_row).transpose()
    }

    pub async fn save_workflow(&self, source: &str) -> Result<(Workflow, bool)> {
        let parsed = self
            .check_pipeline(source)
            .await
            .map_err(|errors| Error::InvalidInput(joined(&errors)))?;
        self.call(move |conn| upsert(conn, &parsed.def, &parsed.source))
            .await
    }

    pub async fn delete_workflow(&self, name: &str) -> Result<String> {
        let name = name.to_string();
        self.call(move |conn| delete(conn, &name)).await
    }

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub id: String,
    pub workflow: String,
    pub snapshot: String,
    pub conversation_id: String,
    pub repo: String,
    pub base_branch: String,
    pub state: String,
    pub input: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunStep {
    pub run_id: String,
    pub step_id: String,
    pub round: u32,
    pub task: Option<String>,
    pub state: String,
    pub verdict: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct StepUpdate<'a> {
    pub state: &'a str,
    pub task: Option<&'a str>,
    pub verdict: Option<&'a str>,
    pub note: Option<&'a str>,
}

impl<'a> StepUpdate<'a> {
    pub fn state(state: &'a str) -> Self {
        Self {
            state,
            task: None,
            verdict: None,
            note: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewRun {
    pub id: String,
    pub workflow: String,
    pub snapshot: String,
    pub conversation_id: String,
    pub repo: String,
    pub base_branch: String,
    pub input: String,
}

pub const RUN_RUNNING: &str = "running";
pub const RUN_NEEDS_YOU: &str = "needsYou";
pub const RUN_FINISHED: &str = "finished";
pub const RUN_FAILED: &str = "failed";

const RUN_COLUMNS: &str =
    "id, workflow, snapshot, conversation_id, repo, base_branch, state, input";

fn run_of(r: &rusqlite::Row<'_>) -> rusqlite::Result<Run> {
    Ok(Run {
        id: r.get(0)?,
        workflow: r.get(1)?,
        snapshot: r.get(2)?,
        conversation_id: r.get(3)?,
        repo: r.get(4)?,
        base_branch: r.get(5)?,
        state: r.get(6)?,
        input: r.get(7)?,
    })
}

fn step_of(r: &rusqlite::Row<'_>) -> rusqlite::Result<RunStep> {
    Ok(RunStep {
        run_id: r.get(0)?,
        step_id: r.get(1)?,
        round: r.get(2)?,
        task: r.get(3)?,
        state: r.get(4)?,
        verdict: r.get(5)?,
        note: r.get(6)?,
    })
}

impl Store {
    pub async fn create_run(&self, new: NewRun) -> Result<Run> {
        self.call(move |conn| {
            let taken = conn
                .query_row("select 1 from runs where id = ?1", [&new.id], |r| {
                    r.get::<_, i64>(0)
                })
                .optional()?
                .is_some();
            if taken {
                return Err(Error::Conflict(format!(
                    "a run named `{}` already exists",
                    new.id
                )));
            }
            conn.execute(
                "insert into runs (id, workflow, snapshot, conversation_id, repo, base_branch, state, input, created_at)
                 values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    new.id,
                    new.workflow,
                    new.snapshot,
                    new.conversation_id,
                    new.repo,
                    new.base_branch,
                    RUN_RUNNING,
                    new.input,
                    clock::now_rfc3339()
                ],
            )?;
            Ok(Run {
                id: new.id,
                workflow: new.workflow,
                snapshot: new.snapshot,
                conversation_id: new.conversation_id,
                repo: new.repo,
                base_branch: new.base_branch,
                state: RUN_RUNNING.into(),
                input: new.input,
            })
        })
        .await
    }

    pub async fn run(&self, id: &str) -> Result<Option<Run>> {
        let id = id.to_string();
        self.call(move |conn| {
            Ok(conn
                .query_row(
                    &format!("select {RUN_COLUMNS} from runs where id = ?1"),
                    [id],
                    run_of,
                )
                .optional()?)
        })
        .await
    }

    pub async fn runs_of(&self, conversation_id: &str) -> Result<Vec<Run>> {
        let conversation_id = conversation_id.to_string();
        self.call(move |conn| {
            let mut stmt = conn.prepare(&format!(
                "select {RUN_COLUMNS} from runs where conversation_id = ?1
                 order by created_at desc, rowid desc"
            ))?;
            let found: rusqlite::Result<Vec<Run>> =
                stmt.query_map([conversation_id], run_of)?.collect();
            Ok(found?)
        })
        .await
    }

    pub async fn runs_in(&self, state: &str) -> Result<Vec<Run>> {
        let state = state.to_string();
        self.call(move |conn| {
            let mut stmt = conn.prepare(&format!(
                "select {RUN_COLUMNS} from runs where state = ?1 order by created_at, id"
            ))?;
            let found: rusqlite::Result<Vec<Run>> = stmt.query_map([state], run_of)?.collect();
            Ok(found?)
        })
        .await
    }

    pub async fn move_run(&self, id: &str, from: &str, to: &str) -> Result<bool> {
        let (id, from, to) = (id.to_string(), from.to_string(), to.to_string());
        self.call(move |conn| {
            let ended = (to != RUN_RUNNING).then(clock::now_rfc3339);
            let changed = conn.execute(
                "update runs set state = ?3, ended_at = ?4 where id = ?1 and state = ?2",
                params![id, from, to, ended],
            )?;
            Ok(changed == 1)
        })
        .await
    }

    pub async fn run_steps(&self, run_id: &str) -> Result<Vec<RunStep>> {
        let run_id = run_id.to_string();
        self.call(move |conn| {
            let mut stmt = conn.prepare(
                "select run_id, step_id, round, task, state, verdict, note from run_steps
                 where run_id = ?1 order by rowid",
            )?;
            let found: rusqlite::Result<Vec<RunStep>> =
                stmt.query_map([run_id], step_of)?.collect();
            Ok(found?)
        })
        .await
    }

    pub async fn claim_run_step(&self, run_id: &str, step_id: &str, round: u32) -> Result<bool> {
        let (run_id, step_id) = (run_id.to_string(), step_id.to_string());
        self.call(move |conn| {
            let changed = conn.execute(
                "insert or ignore into run_steps (run_id, step_id, round, state)
                 values (?1, ?2, ?3, 'starting')",
                params![run_id, step_id, round],
            )?;
            Ok(changed == 1)
        })
        .await
    }

    pub async fn set_run_step(
        &self,
        run_id: &str,
        step_id: &str,
        round: u32,
        update: StepUpdate<'_>,
    ) -> Result<()> {
        let (run_id, step_id) = (run_id.to_string(), step_id.to_string());
        let state = update.state.to_string();
        let (task, verdict, note) = (
            update.task.map(String::from),
            update.verdict.map(String::from),
            update.note.map(String::from),
        );
        self.call(move |conn| {
            conn.execute(
                "update run_steps set state = ?4, task = coalesce(?5, task),
                 verdict = coalesce(?6, verdict), note = coalesce(?7, note)
                 where run_id = ?1 and step_id = ?2 and round = ?3",
                params![run_id, step_id, round, state, task, verdict, note],
            )?;
            Ok(())
        })
        .await
    }

    pub async fn start_run_step(
        &self,
        run_id: &str,
        step_id: &str,
        round: u32,
        task: &str,
    ) -> Result<bool> {
        let (run_id, step_id, task) = (run_id.to_string(), step_id.to_string(), task.to_string());
        self.call(move |conn| {
            let changed = conn.execute(
                "update run_steps set state = 'started', task = ?4
                 where run_id = ?1 and step_id = ?2 and round = ?3 and state = 'starting'",
                params![run_id, step_id, round, task],
            )?;
            Ok(changed == 1)
        })
        .await
    }

    pub async fn run_task_names(&self) -> Result<std::collections::HashSet<String>> {
        self.call(|conn| {
            let mut stmt = conn.prepare("select task from run_steps where task is not null")?;
            let found: rusqlite::Result<std::collections::HashSet<String>> = stmt
                .query_map([], |r| r.get::<_, String>(0).map(|n| n.to_lowercase()))?
                .collect();
            Ok(found?)
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

#[cfg(test)]
mod tests {
    use crate::events::EventBus;
    use crate::store::Store;
    use crate::{Error, Home};

    fn pipeline(name: &str, repos: &[&str], default: bool) -> String {
        let repos: Vec<String> = repos.iter().map(|r| format!("\"{r}\"")).collect();
        format!(
            "name = \"{name}\"\nrepos = [{}]\ndefault = {default}\nwork = \"build\"\n\n[[step]]\nid = \"build\"\nagent = \"builder\"\n",
            repos.join(", ")
        )
    }

    fn store() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let home = Home::at(dir.path().join("h"));
        let store = Store::open(&home, EventBus::new()).unwrap();
        (dir, store)
    }

    async fn default_of(store: &Store, repo: &str) -> Option<String> {
        store
            .default_workflow(repo)
            .await
            .unwrap()
            .map(|w| w.def.name)
    }

    #[tokio::test]
    async fn no_default_means_none() {
        let (_dir, store) = store();
        store
            .save_workflow(&pipeline("plain", &[], false))
            .await
            .unwrap();
        assert_eq!(default_of(&store, "shop").await, None);
    }

    #[tokio::test]
    async fn a_default_for_every_repo_applies_to_any_repo() {
        let (_dir, store) = store();
        store
            .save_workflow(&pipeline("all", &[], true))
            .await
            .unwrap();
        assert_eq!(default_of(&store, "shop").await.as_deref(), Some("all"));
        assert_eq!(default_of(&store, "admin").await.as_deref(), Some("all"));
    }

    #[tokio::test]
    async fn a_default_naming_the_repo_beats_one_for_every_repo() {
        let (_dir, store) = store();
        store
            .save_workflow(&pipeline("all", &[], true))
            .await
            .unwrap();
        store
            .save_workflow(&pipeline("shop-only", &["Shop"], true))
            .await
            .unwrap();
        assert_eq!(
            default_of(&store, "shop").await.as_deref(),
            Some("shop-only")
        );
        assert_eq!(default_of(&store, "admin").await.as_deref(), Some("all"));
    }

    #[tokio::test]
    async fn a_default_for_other_repos_does_not_apply() {
        let (_dir, store) = store();
        store
            .save_workflow(&pipeline("admin-only", &["admin"], true))
            .await
            .unwrap();
        assert_eq!(default_of(&store, "shop").await, None);
    }

    #[tokio::test]
    async fn a_second_default_for_the_same_scope_is_refused() {
        let (_dir, store) = store();
        store
            .save_workflow(&pipeline("one", &[], true))
            .await
            .unwrap();
        let Err(Error::InvalidInput(text)) = store.save_workflow(&pipeline("two", &[], true)).await
        else {
            panic!("expected a refusal");
        };
        assert!(
            text.contains("\"one\" is already the default for every repo"),
            "{text}"
        );
        assert!(store.workflow("two").await.unwrap().is_none());

        store
            .save_workflow(&pipeline("shop-a", &["shop"], true))
            .await
            .unwrap();
        let Err(Error::InvalidInput(text)) = store
            .save_workflow(&pipeline("shop-b", &["admin", "SHOP"], true))
            .await
        else {
            panic!("expected a refusal");
        };
        assert!(text.contains("\"shop-a\" is already the default"), "{text}");
    }

    #[tokio::test]
    async fn saving_a_default_again_and_non_defaults_are_allowed() {
        let (_dir, store) = store();
        store
            .save_workflow(&pipeline("one", &[], true))
            .await
            .unwrap();
        let (_, replaced) = store
            .save_workflow(&pipeline("one", &[], true))
            .await
            .unwrap();
        assert!(replaced);
        store
            .save_workflow(&pipeline("two", &[], false))
            .await
            .unwrap();
        store
            .save_workflow(&pipeline("shop-only", &["shop"], true))
            .await
            .unwrap();
    }
}
