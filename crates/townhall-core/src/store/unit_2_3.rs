//! Unit 2.3 tasks: `task_launches` (the commit and Claude session each task's agent started
//! from, so `merged` can be told from a branch that never got a commit) and the one
//! `tasksCreated` entry per turn. Migrations 0200 to 0299, files `migrations/02xx_<name>.sql`.

use std::collections::HashMap;

use rusqlite::{params, Connection, OptionalExtension, ToSql};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::Mutex;

use super::codec::from_json;
use super::conversations::get;
use super::migrate::Migration;
use super::Store;
use crate::model::{EntryBody, ServerEvent, ThreadEntry};
use crate::scheduler::TaskLaunch;
use crate::Result;

pub const MIGRATIONS: &[Migration] = &[
    Migration {
        id: 201,
        name: "0201_task_launches",
        sql: include_str!("migrations/0201_task_launches.sql"),
    },
    Migration {
        id: 202,
        name: "0202_task_handoffs",
        sql: include_str!("migrations/0202_task_handoffs.sql"),
    },
    Migration {
        id: 203,
        name: "0203_task_ended_at",
        sql: include_str!("migrations/0203_task_ended_at.sql"),
    },
    Migration {
        id: 204,
        name: "0204_task_blocked",
        sql: include_str!("migrations/0204_task_blocked.sql"),
    },
    Migration {
        id: 205,
        name: "0205_task_pr_base",
        sql: include_str!("migrations/0205_task_pr_base.sql"),
    },
    Migration {
        id: 206,
        name: "0206_task_done",
        sql: include_str!("migrations/0206_task_done.sql"),
    },
];

const TASKS_CREATED: &str = "tasksCreated";

/// Serializes `note_task_created`, so tasks one turn creates at the same time still share one
/// entry. One townhall owns the database, so a lock in this process is enough.
static TURN: Mutex<()> = Mutex::const_new(());

#[derive(Deserialize)]
struct Listed {
    tasks: Vec<String>,
}

/// The turn's open `tasksCreated` entry with `task` added and written back, or `None` when
/// the turn has none yet.
fn merge_open(conn: &mut Connection, cid: &str, task: &str) -> Result<Option<ThreadEntry>> {
    get(conn, cid)?;
    let tx = conn.transaction()?;
    let turn_start: u64 = tx.query_row(
        "select coalesce(max(id), 0) from (
             select id from messages where conversation_id = ?1 and author = 'you'
             union all
             select id from events where conversation_id = ?1 and kind = 'event')",
        [cid],
        |r| r.get(0),
    )?;
    let open = tx
        .query_row(
            "select id, data, at from events where conversation_id = ?1 and kind = ?2 and id > ?3
             order by id desc limit 1",
            params![cid, TASKS_CREATED, turn_start],
            |r| Ok((r.get::<_, u64>(0)?, r.get::<_, String>(1)?, r.get(2)?)),
        )
        .optional()?;
    let Some((id, data, at)) = open else {
        return Ok(None);
    };
    let listed: Listed = from_json("events.data", &data)?;
    let tasks = match listed.tasks.iter().any(|t| t == task) {
        true => listed.tasks,
        false => [listed.tasks, vec![task.to_string()]].concat(),
    };
    tx.execute(
        "update events set data = ?3 where conversation_id = ?1 and id = ?2",
        params![cid, id, json!({ "tasks": tasks }).to_string()],
    )?;
    tx.commit()?;
    let body = EntryBody::TasksCreated { tasks };
    Ok(Some(ThreadEntry { id, at, body }))
}

const LAUNCH_COLUMNS: &str = "name, start_commit, session_id, launched_at";

fn launches(conn: &Connection, filter: &str, args: &[&dyn ToSql]) -> Result<Vec<TaskLaunch>> {
    let sql = format!("select {LAUNCH_COLUMNS} from task_launches {filter} order by launched_at");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(args, |r| {
        Ok(TaskLaunch {
            name: r.get(0)?,
            start_commit: r.get(1)?,
            session_id: r.get(2)?,
            launched_at: r.get(3)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<TaskLaunch>>>()?)
}

impl Store {
    /// Records that `task` was created in the current turn: one `tasksCreated` entry per turn.
    /// A turn is everything after your latest message or task event (the whole thread before the first).
    /// The turn's first task appends the entry; each later one is added to that entry in
    /// place (same id and time), which is sent again as `entryAppended`, so a client replaces
    /// the entry it has with that id.
    pub async fn note_task_created(
        &self,
        conversation_id: &str,
        task: &str,
    ) -> Result<ThreadEntry> {
        let _turn = TURN.lock().await;
        let (cid, name, bus) = (
            conversation_id.to_string(),
            task.to_string(),
            self.bus().clone(),
        );
        let merged = self
            .call(move |conn| {
                let merged = merge_open(conn, &cid, &name)?;
                if let Some(entry) = &merged {
                    bus.publish(ServerEvent::EntryAppended {
                        conversation_id: cid,
                        entry: entry.clone(),
                    });
                }
                Ok(merged)
            })
            .await?;
        match merged {
            Some(entry) => Ok(entry),
            None => {
                let tasks = vec![task.to_string()];
                let body = EntryBody::TasksCreated { tasks };
                self.append_entry(conversation_id, body).await
            }
        }
    }

    /// Saves (or replaces) what a task's agent was started from.
    pub async fn record_launch(&self, launch: TaskLaunch) -> Result<()> {
        self.call(move |conn| {
            conn.execute(
                "insert or replace into task_launches (name, start_commit, session_id, launched_at)
                 values (?1, ?2, ?3, ?4)",
                params![
                    launch.name,
                    launch.start_commit,
                    launch.session_id,
                    launch.launched_at
                ],
            )?;
            Ok(())
        })
        .await
    }

    /// What the task called `name` (ignoring case) was started from, once it was.
    pub async fn task_launch(&self, name: &str) -> Result<Option<TaskLaunch>> {
        let name = name.to_string();
        self.call(move |conn| {
            Ok(launches(conn, "where name = ?1", &[&name])?
                .into_iter()
                .next())
        })
        .await
    }

    pub async fn task_ended_times(&self) -> Result<HashMap<String, String>> {
        self.call(|conn| {
            let mut stmt =
                conn.prepare("select name, ended_at from tasks where ended_at is not null")?;
            let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
            Ok(rows.collect::<rusqlite::Result<HashMap<String, String>>>()?)
        })
        .await
    }

    /// Every recorded launch, oldest first.
    pub async fn task_launches(&self) -> Result<Vec<TaskLaunch>> {
        self.call(|conn| launches(conn, "", &[])).await
    }
}
