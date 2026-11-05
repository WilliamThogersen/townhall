//! Queued tasks of an ended conversation never start: `queued` to `ended`, with an entry.

use crate::model::{AgentEvent, EntryBody, Task, TaskState};
use crate::store::Store;
use crate::{Error, Result};

/// The `agent` `ended` entry of a queued task cancelled by its conversation's end.
pub const CANCELLED: &str = "cancelled: its conversation ended";
const MOVED: &str = "the task's state moved on";

/// `queued` to `ended`, with an entry saying why. False when it moved on meanwhile.
async fn cancel(store: &Store, task: &Task) -> Result<bool> {
    let change = |t: Task| match t.state {
        TaskState::Queued => Ok(Task {
            state: TaskState::Ended,
            ..t
        }),
        _ => Err(Error::Conflict(MOVED.into())),
    };
    match store.update_task(&task.name, change).await {
        Ok(_) => {}
        Err(Error::Conflict(m)) if m == MOVED => return Ok(false),
        Err(e) => return Err(e),
    }
    let body = EntryBody::Agent {
        agent: task.name.clone(),
        event: AgentEvent::Ended,
        text: CANCELLED.into(),
    };
    store.append_entry(&task.conversation_id, body).await?;
    Ok(true)
}

/// Cancels every queued task of `tasks`. Returns their names.
pub async fn cancel_queued(store: &Store, tasks: &[Task]) -> Result<Vec<String>> {
    let mut cancelled = vec![];
    for task in tasks.iter().filter(|t| t.state == TaskState::Queued) {
        if cancel(store, task).await? {
            cancelled.push(task.name.clone());
        }
    }
    Ok(cancelled)
}
