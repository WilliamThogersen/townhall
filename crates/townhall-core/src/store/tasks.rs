//! The `tasks` table. Names are unique across every conversation and repo, ignoring case.

use rusqlite::{params, Connection, OptionalExtension, ToSql, Transaction};

use super::codec::{from_json, name_of, parse_name, to_json};
use super::conversations::{get, publish};
use super::Store;
use crate::cmd::validate_name;
use crate::model::{NewTask, Pr, ServerEvent, Task, TaskState};
use crate::{clock, Error, Result};

const COLUMNS: &str = "name, conversation_id, title, prompt, repo, base_branch, after, state, \
                       attachments, pr_number, pr_url, pr_checks, created_at, agent";

type Row = (
    [String; 8],
    String,
    Option<u64>,
    Option<String>,
    Option<String>,
    [String; 2],
);

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Row> {
    let text = |i: usize| r.get::<_, String>(i);
    Ok((
        [
            text(0)?,
            text(1)?,
            text(2)?,
            text(3)?,
            text(4)?,
            text(5)?,
            text(6)?,
            text(7)?,
        ],
        text(8)?,
        r.get(9)?,
        r.get(10)?,
        r.get(11)?,
        [text(12)?, text(13)?],
    ))
}

fn build(row: Row) -> Result<Task> {
    let (
        [name, conversation_id, title, prompt, repo, base_branch, after, state],
        attachments,
        number,
        url,
        checks,
        [created_at, agent],
    ) = row;
    let pr = match (number, checks) {
        (Some(number), Some(checks)) => Some(Pr {
            number,
            url,
            checks: parse_name("tasks.pr_checks", &checks)?,
        }),
        _ => None,
    };
    Ok(Task {
        after: from_json("tasks.after", &after)?,
        state: parse_name("tasks.state", &state)?,
        attachments: from_json("tasks.attachments", &attachments)?,
        name,
        conversation_id,
        title,
        prompt,
        repo,
        base_branch,
        created_at,
        pr,
        agent,
    })
}

fn select(conn: &Connection, filter: &str, args: &[&dyn ToSql]) -> Result<Vec<Task>> {
    let sql = format!("select {COLUMNS} from tasks {filter} order by created_at, rowid");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(args, row)?
        .collect::<rusqlite::Result<Vec<Row>>>()?;
    rows.into_iter().map(build).collect()
}

fn find(conn: &Connection, name: &str) -> Result<Option<Task>> {
    Ok(select(conn, "where name = ?1", &[&name])?
        .into_iter()
        .next())
}

fn dedup(names: &[String]) -> Vec<String> {
    names.iter().fold(vec![], |acc, n| {
        if acc.contains(n) {
            acc
        } else {
            acc.into_iter().chain([n.clone()]).collect()
        }
    })
}

/// The `after` names as stored: each must be another existing task (matched ignoring case).
fn resolve_after(tx: &Transaction<'_>, new: &NewTask) -> Result<Vec<String>> {
    let mut after = vec![];
    for name in &new.after {
        if name.eq_ignore_ascii_case(&new.name) {
            return Err(Error::InvalidInput(format!(
                "task `{name}` cannot wait for itself"
            )));
        }
        let found = find(tx, name)?.ok_or_else(|| {
            Error::InvalidInput(format!("`after` names an unknown task `{name}`"))
        })?;
        after.push(found.name);
    }
    Ok(after)
}

/// Checks the new task and returns its `after` list with each task's stored name.
fn check_new(tx: &Transaction<'_>, conversation_id: &str, new: &NewTask) -> Result<Vec<String>> {
    validate_name(&new.name)?;
    if new.title.trim().is_empty() {
        return Err(Error::InvalidInput("a task needs a title".into()));
    }
    if find(tx, &new.name)?.is_some() {
        return Err(Error::Conflict(format!(
            "a task named `{}` already exists",
            new.name
        )));
    }
    let after = resolve_after(tx, new)?;
    for id in &new.attachments {
        let known: Option<i64> = tx
            .query_row(
                "select 1 from attachments where id = ?1 and conversation_id = ?2",
                params![id, conversation_id],
                |r| r.get(0),
            )
            .optional()?;
        if known.is_none() {
            return Err(Error::InvalidInput(format!("unknown attachment `{id}`")));
        }
    }
    Ok(after)
}

fn insert(tx: &Transaction<'_>, t: &Task) -> Result<()> {
    tx.execute(
        "insert into tasks (name, conversation_id, title, prompt, repo, base_branch, after, state,
         attachments, created_at, agent) values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            t.name,
            t.conversation_id,
            t.title,
            t.prompt,
            t.repo,
            t.base_branch,
            to_json(&t.after)?,
            name_of(&t.state),
            to_json(&t.attachments)?,
            t.created_at,
            t.agent
        ],
    )?;
    Ok(())
}

fn write(tx: &Transaction<'_>, t: &Task) -> Result<()> {
    let (number, url, checks) = match &t.pr {
        Some(pr) => (Some(pr.number), pr.url.clone(), Some(name_of(&pr.checks))),
        None => (None, None, None),
    };
    tx.execute(
        "update tasks set title = ?2, prompt = ?3, repo = ?4, base_branch = ?5, after = ?6,
         state = ?7, attachments = ?8, pr_number = ?9, pr_url = ?10, pr_checks = ?11,
         agent = ?12 where name = ?1",
        params![
            t.name,
            t.title,
            t.prompt,
            t.repo,
            t.base_branch,
            to_json(&t.after)?,
            name_of(&t.state),
            to_json(&t.attachments)?,
            number,
            url,
            checks,
            t.agent
        ],
    )?;
    Ok(())
}

fn queued(conversation_id: &str, new: NewTask) -> Task {
    Task {
        after: dedup(&new.after),
        attachments: dedup(&new.attachments),
        name: new.name,
        title: new.title,
        prompt: new.prompt,
        repo: new.repo,
        base_branch: new.base_branch,
        state: TaskState::Queued,
        conversation_id: conversation_id.to_string(),
        created_at: clock::now_rfc3339(),
        pr: None,
        agent: new.agent,
    }
}

impl Store {
    /// A queued task. `InvalidName`, `Conflict` for a taken name (ignoring case),
    /// `InvalidInput` for an unknown `after` task or attachment. Adds it to the conversation
    /// and publishes `taskUpdated`, then `conversationUpdated`.
    pub async fn create_task(&self, conversation_id: &str, new: NewTask) -> Result<Task> {
        let (cid, bus) = (conversation_id.to_string(), self.bus().clone());
        self.call(move |conn| {
            get(conn, &cid)?;
            let tx = conn.transaction()?;
            let after = check_new(&tx, &cid, &new)?;
            let task = queued(&cid, NewTask { after, ..new });
            insert(&tx, &task)?;
            tx.execute(
                "update conversations set updated_at = ?2 where id = ?1",
                params![cid, task.created_at],
            )?;
            tx.commit()?;
            bus.publish(ServerEvent::TaskUpdated { task: task.clone() });
            publish(&bus, &get(conn, &cid)?);
            Ok(task)
        })
        .await
    }

    /// One conversation's tasks, oldest first.
    pub async fn tasks(&self, conversation_id: &str) -> Result<Vec<Task>> {
        let cid = conversation_id.to_string();
        self.call(move |conn| {
            get(conn, &cid)?;
            select(conn, "where conversation_id = ?1", &[&cid])
        })
        .await
    }

    /// Every task, oldest first.
    pub async fn all_tasks(&self) -> Result<Vec<Task>> {
        self.call(|conn| select(conn, "", &[])).await
    }

    /// The task called `name`, ignoring case.
    pub async fn find_task(&self, name: &str) -> Result<Option<Task>> {
        let name = name.to_string();
        self.call(move |conn| find(conn, &name)).await
    }

    /// Applies `change` to a copy and saves it; `name`, `conversationId` and `createdAt`
    /// are kept. Publishes `taskUpdated`.
    pub async fn update_task<F>(&self, name: &str, change: F) -> Result<Task>
    where
        F: FnOnce(Task) -> Result<Task> + Send + 'static,
    {
        let (name, bus) = (name.to_string(), self.bus().clone());
        self.call(move |conn| {
            let tx = conn.transaction()?;
            let current = find(&tx, &name)?.ok_or_else(|| Error::missing("task", &name))?;
            let keep = (
                current.name.clone(),
                current.conversation_id.clone(),
                current.created_at.clone(),
            );
            let next = Task {
                name: keep.0,
                conversation_id: keep.1,
                created_at: keep.2,
                ..change(current)?
            };
            write(&tx, &next)?;
            tx.commit()?;
            bus.publish(ServerEvent::TaskUpdated { task: next.clone() });
            Ok(next)
        })
        .await
    }
}
