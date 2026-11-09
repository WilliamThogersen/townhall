//! `task_handoffs`: each time an agent handed its task back to another agent type. A hand
//! back is one write: the row, the task queued again as the target type, and its thread entry.

use rusqlite::{params, Connection, OptionalExtension, Transaction};

use super::conversations::{get, publish};
use super::tasks::{find, write};
use super::thread::{insert, next_id};
use super::Store;
use crate::model::{
    AgentEvent, Done, EntryBody, Handoff, ServerEvent, Task, TaskState, ThreadEntry, Verify,
    VerifyPhase, MAX_HANDOFFS,
};
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

/// Which tasks a hand back may take: the running ones (an agent hands its own task back), or
/// the ended ones (townhall hands a finished task on to a reflector).
#[derive(Debug, Clone, Copy)]
enum Takes {
    Running,
    Ended,
}

impl Takes {
    fn allows(self, state: TaskState) -> bool {
        match self {
            Self::Running => matches!(state, TaskState::Working | TaskState::NeedsYou),
            Self::Ended => state == TaskState::Ended,
        }
    }

    fn refusal(self, task: &str) -> String {
        match self {
            Self::Running => {
                format!("task `{task}` is not running: only a running task can be handed back")
            }
            Self::Ended => {
                format!("task `{task}` has not ended: only an ended task can be handed on")
            }
        }
    }
}

fn hand_back(
    conn: &mut Connection,
    name: &str,
    to: &str,
    reason: &str,
    summary: &str,
    from: Takes,
) -> Result<(Task, Handoff, ThreadEntry)> {
    let tx = conn.transaction()?;
    let task = find(&tx, name)?.ok_or_else(|| Error::missing("task", name))?;
    if !from.allows(task.state) {
        return Err(Error::Conflict(from.refusal(&task.name)));
    }
    let done = listed(&tx, &task.name)?;
    if done.len() >= MAX_HANDOFFS {
        return Err(Error::Conflict(format!(
            "task `{}` was already handed back {MAX_HANDOFFS} times: stop and ask the user instead",
            task.name
        )));
    }
    let result = requeue(&tx, task, to, reason, summary, done.len(), None)?;
    tx.commit()?;
    Ok(result)
}

/// Records the hand back, queues the task again as `to` and adds its thread entry. `verify`
/// replaces the task's verify state when given (a retry after a failed check).
fn requeue(
    tx: &Transaction,
    task: Task,
    to: &str,
    reason: &str,
    summary: &str,
    seq: usize,
    verify: Option<Verify>,
) -> Result<(Task, Handoff, ThreadEntry)> {
    let to = target(tx, to)?;
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
            seq,
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
        done: Done {
            verify: verify.or(task.done.verify.clone()),
            ..task.done.clone()
        },
        ..task
    };
    write(tx, &next)?;
    let entry = ThreadEntry {
        id: next_id(tx, &next.conversation_id, &handoff.at)?,
        at: handoff.at.clone(),
        body: EntryBody::Handoff {
            task: next.name.clone(),
            from: handoff.from.clone(),
            to: handoff.to.clone(),
            reason: handoff.reason.clone(),
        },
    };
    insert(tx, &next.conversation_id, &entry)?;
    Ok((next, handoff, entry))
}

/// A task whose check failed: queued again for its own agent type (a hand back to itself, so
/// the output reaches its brief) while retries are left, else it needs the user.
fn check_failed(conn: &mut Connection, name: &str, output: &str) -> Result<(Task, ThreadEntry)> {
    let tx = conn.transaction()?;
    let task = find(&tx, name)?.ok_or_else(|| Error::missing("task", name))?;
    let running = matches!(task.state, TaskState::Working | TaskState::NeedsYou);
    let verify = task
        .done
        .verify
        .clone()
        .filter(|v| running && v.phase == VerifyPhase::Verifying);
    let Some(verify) = verify else {
        return Err(Error::Conflict(format!(
            "task `{}` is not under verification",
            task.name
        )));
    };
    let handed = listed(&tx, &task.name)?.len();
    let (retries, max) = (verify.retries, task.done.max_retries);
    let result = if retries < max && handed < MAX_HANDOFFS {
        let next = Verify {
            phase: VerifyPhase::Retrying,
            retries: retries + 1,
            output: Some(output.to_string()),
        };
        let command = task.done.check.clone().unwrap_or_default();
        let reason = format!(
            "the check `{command}` failed (retry {}/{max}). Its output:\n{output}",
            retries + 1
        );
        let to = task.agent.clone();
        let (next, _, entry) = requeue(&tx, task, &to, &reason, "", handed, Some(next))?;
        (next, entry)
    } else {
        let at = clock::now_rfc3339();
        let failed = Verify {
            phase: VerifyPhase::Failed,
            retries,
            output: Some(output.to_string()),
        };
        let next = Task {
            state: TaskState::NeedsYou,
            done: Done {
                verify: Some(failed),
                ..task.done.clone()
            },
            ..task
        };
        write(&tx, &next)?;
        let entry = ThreadEntry {
            id: next_id(&tx, &next.conversation_id, &at)?,
            at,
            body: EntryBody::Agent {
                agent: next.name.clone(),
                event: AgentEvent::Failed,
                text: format!("the check failed after {retries} retries. Its output:\n{output}"),
            },
        };
        insert(&tx, &next.conversation_id, &entry)?;
        (next, entry)
    };
    tx.commit()?;
    Ok(result)
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
        self.hand_over(name, to, reason, summary, Takes::Running)
            .await
    }

    /// Like [`Store::hand_back`] for an `ended` task: it is queued again as `to`, in its own
    /// worktree and branch. `Conflict` when it has not ended or was handed back
    /// `MAX_HANDOFFS` times.
    pub async fn hand_on(
        &self,
        name: &str,
        to: &str,
        reason: &str,
        summary: &str,
    ) -> Result<Handoff> {
        self.hand_over(name, to, reason, summary, Takes::Ended)
            .await
    }

    async fn hand_over(
        &self,
        name: &str,
        to: &str,
        reason: &str,
        summary: &str,
        from: Takes,
    ) -> Result<Handoff> {
        let args = [name, to, reason, summary].map(String::from);
        let bus = self.bus().clone();
        self.call(move |conn| {
            let [name, to, reason, summary] = &args;
            let (task, handoff, entry) = hand_back(conn, name, to, reason, summary, from)?;
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

    /// The check of the verifying task `name` failed with `output`: it is queued again for its
    /// own agent type with the output in its brief while it has retries left, else it needs the
    /// user (`verify` `failed`). `Conflict` when the task is not under verification.
    /// Publishes `taskUpdated`, `entryAppended` and `conversationUpdated`.
    pub async fn check_failed(&self, name: &str, output: &str) -> Result<Task> {
        let (name, output) = (name.to_string(), output.to_string());
        let bus = self.bus().clone();
        self.call(move |conn| {
            let (task, entry) = check_failed(conn, &name, &output)?;
            bus.publish(ServerEvent::TaskUpdated { task: task.clone() });
            bus.publish(ServerEvent::EntryAppended {
                conversation_id: task.conversation_id.clone(),
                entry,
            });
            publish(&bus, &get(conn, &task.conversation_id)?);
            Ok(task)
        })
        .await
    }

    /// The task's hand backs, oldest first.
    pub async fn handoffs(&self, task: &str) -> Result<Vec<Handoff>> {
        let task = task.to_string();
        self.call(move |conn| listed(conn, &task)).await
    }
}
