//! The `conversations` table. `tasks` is not a column: it is read from `tasks`.

use rusqlite::{params, Connection, OptionalExtension};

use super::codec::{name_of, parse_name};
use super::Store;
use crate::events::EventBus;
use crate::model::{
    ContextUsage, Conversation, ConversationDetail, ConversationStatus, ServerEvent,
    NEW_CONVERSATION_TITLE,
};
use crate::{clock, ids, Error, Result};

const COLUMNS: &str = "id, session_id, session_started, title, status, linked_repo, ended, \
                       context_used, context_limit, created_at, updated_at";
const ID_ATTEMPTS: usize = 8;

struct Row {
    id: String,
    session_id: String,
    session_started: bool,
    title: String,
    status: String,
    linked_repo: Option<String>,
    ended: bool,
    context: (Option<u64>, Option<u64>),
    created_at: String,
    updated_at: String,
}

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Row> {
    Ok(Row {
        id: r.get(0)?,
        session_id: r.get(1)?,
        session_started: r.get(2)?,
        title: r.get(3)?,
        status: r.get(4)?,
        linked_repo: r.get(5)?,
        ended: r.get(6)?,
        context: (r.get(7)?, r.get(8)?),
        created_at: r.get(9)?,
        updated_at: r.get(10)?,
    })
}

fn task_names(conn: &Connection, id: &str) -> Result<Vec<String>> {
    let mut stmt = conn
        .prepare("select name from tasks where conversation_id = ?1 order by created_at, rowid")?;
    let names = stmt.query_map([id], |r| r.get(0))?;
    Ok(names.collect::<rusqlite::Result<Vec<String>>>()?)
}

fn build(conn: &Connection, row: Row) -> Result<Conversation> {
    let context = match row.context {
        (Some(used), Some(limit)) => Some(ContextUsage { used, limit }),
        _ => None,
    };
    Ok(Conversation {
        tasks: task_names(conn, &row.id)?,
        status: parse_name("conversations.status", &row.status)?,
        id: row.id,
        session_id: row.session_id,
        session_started: row.session_started,
        title: row.title,
        linked_repo: row.linked_repo,
        ended: row.ended,
        context,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

pub(super) fn get(conn: &Connection, id: &str) -> Result<Conversation> {
    ids::validate_id("conversation", id)?;
    let sql = format!("select {COLUMNS} from conversations where id = ?1");
    let found = conn.query_row(&sql, [id], row).optional()?;
    let found = found.ok_or_else(|| Error::missing("conversation", id))?;
    build(conn, found)
}

pub(super) fn publish(bus: &EventBus, conversation: &Conversation) {
    bus.publish(ServerEvent::ConversationUpdated {
        conversation: conversation.clone(),
    });
}

fn write(conn: &Connection, c: &Conversation) -> Result<()> {
    let (used, limit) = c
        .context
        .map_or((None, None), |u| (Some(u.used), Some(u.limit)));
    conn.execute(
        "update conversations set session_id = ?2, session_started = ?3, title = ?4, status = ?5,
         linked_repo = ?6, ended = ?7, context_used = ?8, context_limit = ?9, updated_at = ?10
         where id = ?1",
        params![
            c.id,
            c.session_id,
            c.session_started,
            c.title,
            name_of(&c.status),
            c.linked_repo,
            c.ended,
            used,
            limit,
            c.updated_at
        ],
    )?;
    Ok(())
}

fn insert_new(conn: &Connection) -> Result<Conversation> {
    let now = clock::now_rfc3339();
    for _ in 0..ID_ATTEMPTS {
        let (id, session_id) = (ids::new_id(), ids::new_session_id());
        let added = conn.execute(
            "insert into conversations (id, session_id, title, status, created_at, updated_at)
             values (?1, ?2, ?3, ?4, ?5, ?5) on conflict do nothing",
            params![
                id,
                session_id,
                NEW_CONVERSATION_TITLE,
                name_of(&ConversationStatus::YourTurn),
                now
            ],
        )?;
        if added == 1 {
            return get(conn, &id);
        }
    }
    Err(Error::Conflict("no free conversation id, try again".into()))
}

impl Store {
    /// Fresh id and session id, `new conversation`, your turn. Publishes `conversationUpdated`.
    pub async fn create_conversation(&self) -> Result<Conversation> {
        let bus = self.bus().clone();
        self.call(move |conn| {
            let created = insert_new(conn)?;
            publish(&bus, &created);
            Ok(created)
        })
        .await
    }

    /// `Missing` for an unknown id, `InvalidInput` for an unsafe one.
    pub async fn conversation(&self, id: &str) -> Result<Conversation> {
        let id = id.to_string();
        self.call(move |conn| get(conn, &id)).await
    }

    /// Newest activity first.
    pub async fn conversations(&self) -> Result<Vec<Conversation>> {
        self.call(|conn| {
            let sql = format!(
                "select {COLUMNS} from conversations order by updated_at desc, created_at desc"
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map([], row)?
                .collect::<rusqlite::Result<Vec<Row>>>()?;
            rows.into_iter().map(|r| build(conn, r)).collect()
        })
        .await
    }

    /// Applies `change` to a copy and saves it. `id`, `createdAt` and `tasks` are kept,
    /// `updatedAt` is now. Publishes `conversationUpdated`.
    pub async fn update_conversation<F>(&self, id: &str, change: F) -> Result<Conversation>
    where
        F: FnOnce(Conversation) -> Result<Conversation> + Send + 'static,
    {
        let (id, bus) = (id.to_string(), self.bus().clone());
        self.call(move |conn| {
            let tx = conn.transaction()?;
            let current = get(&tx, &id)?;
            let (created_at, tasks) = (current.created_at.clone(), current.tasks.clone());
            let next = Conversation {
                id: id.clone(),
                created_at,
                tasks,
                updated_at: clock::now_rfc3339(),
                ..change(current)?
            };
            write(&tx, &next)?;
            tx.commit()?;
            publish(&bus, &next);
            Ok(next)
        })
        .await
    }

    /// `GET /api/conversations/{id}` in one call.
    pub async fn detail(&self, id: &str) -> Result<ConversationDetail> {
        Ok(ConversationDetail {
            conversation: self.conversation(id).await?,
            entries: self.entries(id).await?,
            tasks: self.tasks(id).await?,
            attachments: self.attachments(id).await?,
        })
    }
}
