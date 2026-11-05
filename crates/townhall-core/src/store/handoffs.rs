//! `task_handoffs`: each time an agent handed its task back to another agent type. A hand
//! back is one write: the row, the task queued again as the target type, and its thread entry.

use rusqlite::{params, Connection, OptionalExtension};

use super::conversations::{get, publish};
use super::tasks::{find, write};
use super::thread::{insert, next_id};
use super::Store;
use crate::model::{EntryBody, Handoff, ServerEvent, Task, TaskState, ThreadEntry, MAX_HANDOFFS};
use crate::{clock, Error, Result};

fn listed(conn: &Connection, task: &str) -> Result<Vec<Handoff>> {
    let mut stmt = conn.prepare(
        "select task, from_type, to_type, reason, summary, at from task_handoffs
         where task = ?1 order by seq",
    )?;
    let rows = stmt.query_map([task], |r| {
        Ok(Handoff {
            task: r.get(0)?,
            from: r.get(1)?,
            to: r.get(2)?,
            reason: r.get(3)?,
            summary: r.get(4)?,
            at: r.get(5)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<Handoff>>>()?)
}

fn type_names(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("select name from agents order by name collate nocase")?;
    let rows = stmt.query_map([], |r| r.get(0))?;
    Ok(rows.collect::<rusqlite::Result<Vec<String>>>()?)
}

fn target(conn: &Connection, to: &str) -> Result<String> {
    let found: Option<String> = conn
        .query_row("select name from agents where name = ?1", [to], |r| {
            r.get(0)
        })
        .optional()?;
    found.ok_or_else(|| {
        Error::InvalidInput(format!(
            "unknown agent type `{to}`: use one of {}",
            type_names(conn).unwrap_or_default().join(", ")
        ))
    })
}

fn hand_back(
    conn: &mut Connection,
    name: &str,
    to: &str,
    reason: &str,
    summary: &str,
) -> Result<(Task, Handoff, ThreadEntry)> {
    let tx = conn.transaction()?;
    let task = find(&tx, name)?.ok_or_else(|| Error::missing("task", name))?;
    if !matches!(task.state, TaskState::Working | TaskState::NeedsYou) {
        return Err(Error::Conflict(format!(
            "task `{}` is not running: only a running task can be handed back",
            task.name
        )));
    }
    let done = listed(&tx, &task.name)?;
    if done.len() >= MAX_HANDOFFS {
        return Err(Error::Conflict(format!(
            "task `{}` was already handed back {MAX_HANDOFFS} times: stop and ask the user instead",
            task.name
        )));
    }
    let to = target(&tx, to)?;
    let handoff = Handoff {
        task: task.name.clone(),
        from: task.agent.clone(),
        to: to.clone(),
        reason: reason.to_string(),
        summary: summary.to_string(),
        at: clock::now_rfc3339(),
    };
    tx.execute(
        "insert into task_handoffs (task, seq, from_type, to_type, reason, summary, at)
         values (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            handoff.task,
            done.len(),
            handoff.from,
            handoff.to,
            handoff.reason,
            handoff.summary,
            handoff.at
        ],
    )?;
    let next = Task {
        agent: to,
        state: TaskState::Queued,
        ..task
    };
    write(&tx, &next)?;
    let entry = ThreadEntry {
        id: next_id(&tx, &next.conversation_id, &handoff.at)?,
        at: handoff.at.clone(),
        body: EntryBody::Handoff {
            task: next.name.clone(),
            from: handoff.from.clone(),
            to: handoff.to.clone(),
            reason: handoff.reason.clone(),
        },
    };
    insert(&tx, &next.conversation_id, &entry)?;
    tx.commit()?;
    Ok((next, handoff, entry))
}

impl Store {
    /// Hands the running task `name` back to the saved agent type `to`: it is queued again as
    /// that type, with a `handoff` entry in its conversation. `Conflict` when it is not
    /// running or was already handed back `MAX_HANDOFFS` times, `InvalidInput` for an
    /// unknown type. Publishes `taskUpdated`, `entryAppended` and `conversationUpdated`.
    pub async fn hand_back(
        &self,
        name: &str,
        to: &str,
        reason: &str,
        summary: &str,
    ) -> Result<Handoff> {
        let args = [name, to, reason, summary].map(String::from);
        let bus = self.bus().clone();
        self.call(move |conn| {
            let [name, to, reason, summary] = &args;
            let (task, handoff, entry) = hand_back(conn, name, to, reason, summary)?;
            bus.publish(ServerEvent::TaskUpdated { task: task.clone() });
            bus.publish(ServerEvent::EntryAppended {
                conversation_id: task.conversation_id.clone(),
                entry,
            });
            publish(&bus, &get(conn, &task.conversation_id)?);
            Ok(handoff)
        })
        .await
    }

    /// The task's hand backs, oldest first.
    pub async fn handoffs(&self, task: &str) -> Result<Vec<Handoff>> {
        let task = task.to_string();
        self.call(move |conn| listed(conn, &task)).await
    }
}
