//! Thread entries: your and townhall's messages in `messages`, everything else in `events`
//! (its body as JSON). Both share the conversation's entry ids (1, 2, 3 ...).

use rusqlite::{params, Connection, Transaction};
use serde_json::Value;

use super::codec::{from_json, to_json};
use super::conversations::{get, publish};
use super::Store;
use crate::model::{EntryBody, ServerEvent, ThreadEntry};
use crate::{clock, Error, Result};

const YOU: &str = "you";
const TOWNHALL: &str = "townhall";

fn next_id(tx: &Transaction<'_>, conversation_id: &str, at: &str) -> Result<u64> {
    let changed = tx.execute(
        "update conversations set last_entry_id = last_entry_id + 1, updated_at = ?2 where id = ?1",
        params![conversation_id, at],
    )?;
    if changed == 0 {
        return Err(Error::missing("conversation", conversation_id));
    }
    Ok(tx.query_row(
        "select last_entry_id from conversations where id = ?1",
        [conversation_id],
        |r| r.get(0),
    )?)
}

fn check_attachments(tx: &Transaction<'_>, conversation_id: &str, ids: &[String]) -> Result<()> {
    for id in ids {
        let known: bool = tx.query_row(
            "select exists (select 1 from attachments where id = ?1 and conversation_id = ?2)",
            params![id, conversation_id],
            |r| r.get(0),
        )?;
        if !known {
            return Err(Error::InvalidInput(format!("unknown attachment `{id}`")));
        }
    }
    Ok(())
}

fn insert(tx: &Transaction<'_>, conversation_id: &str, entry: &ThreadEntry) -> Result<()> {
    let message = |author: &str, text: &str, attachments: &[String]| -> Result<()> {
        tx.execute(
            "insert into messages (conversation_id, id, author, text, attachments, at)
             values (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                conversation_id,
                entry.id,
                author,
                text,
                to_json(&attachments)?,
                entry.at
            ],
        )?;
        Ok(())
    };
    match &entry.body {
        EntryBody::You { text, attachments } => {
            check_attachments(tx, conversation_id, attachments)?;
            message(YOU, text, attachments)
        }
        EntryBody::Townhall { text } => message(TOWNHALL, text, &[]),
        other => {
            let mut data =
                serde_json::to_value(other).map_err(|e| Error::InvalidInput(e.to_string()))?;
            let kind = data.as_object_mut().and_then(|o| o.remove("kind"));
            let kind = kind
                .and_then(|k| k.as_str().map(String::from))
                .unwrap_or_default();
            tx.execute(
                "insert into events (conversation_id, id, kind, data, at) values (?1, ?2, ?3, ?4, ?5)",
                params![conversation_id, entry.id, kind, data.to_string(), entry.at],
            )?;
            Ok(())
        }
    }
}

fn messages(conn: &Connection, conversation_id: &str) -> Result<Vec<ThreadEntry>> {
    let mut stmt = conn.prepare(
        "select id, author, text, attachments, at from messages where conversation_id = ?1",
    )?;
    let rows = stmt.query_map([conversation_id], |r| {
        Ok((
            r.get::<_, u64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, String>(4)?,
        ))
    })?;
    let mut out = vec![];
    for row in rows {
        let (id, author, text, attachments, at) = row?;
        let body = match author.as_str() {
            YOU => EntryBody::You {
                text,
                attachments: from_json("messages.attachments", &attachments)?,
            },
            _ => EntryBody::Townhall { text },
        };
        out.push(ThreadEntry { id, at, body });
    }
    Ok(out)
}

fn events(conn: &Connection, conversation_id: &str) -> Result<Vec<ThreadEntry>> {
    let mut stmt =
        conn.prepare("select id, kind, data, at from events where conversation_id = ?1")?;
    let rows = stmt.query_map([conversation_id], |r| {
        Ok((
            r.get::<_, u64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
        ))
    })?;
    let mut out = vec![];
    for row in rows {
        let (id, kind, data, at) = row?;
        let mut value: Value = from_json("events.data", &data)?;
        if let Some(object) = value.as_object_mut() {
            object.insert("kind".into(), Value::String(kind));
        }
        let body = serde_json::from_value(value).map_err(|e| Error::Corrupt {
            path: "townhall.db column events.data".into(),
            message: e.to_string(),
        })?;
        out.push(ThreadEntry { id, at, body });
    }
    Ok(out)
}

impl Store {
    /// The next entry id and the current time; bumps the conversation's `updatedAt`.
    /// Publishes `entryAppended`, then `conversationUpdated`.
    pub async fn append_entry(
        &self,
        conversation_id: &str,
        body: EntryBody,
    ) -> Result<ThreadEntry> {
        let (cid, bus) = (conversation_id.to_string(), self.bus().clone());
        self.call(move |conn| {
            get(conn, &cid)?;
            let tx = conn.transaction()?;
            let at = clock::now_rfc3339();
            let entry = ThreadEntry {
                id: next_id(&tx, &cid, &at)?,
                at,
                body,
            };
            insert(&tx, &cid, &entry)?;
            tx.commit()?;
            bus.publish(ServerEvent::EntryAppended {
                conversation_id: cid.clone(),
                entry: entry.clone(),
            });
            publish(&bus, &get(conn, &cid)?);
            Ok(entry)
        })
        .await
    }

    /// The thread, oldest first.
    pub async fn entries(&self, conversation_id: &str) -> Result<Vec<ThreadEntry>> {
        let cid = conversation_id.to_string();
        self.call(move |conn| {
            get(conn, &cid)?;
            let mut all: Vec<ThreadEntry> = messages(conn, &cid)?
                .into_iter()
                .chain(events(conn, &cid)?)
                .collect();
            all.sort_by_key(|e| e.id);
            Ok(all)
        })
        .await
    }
}
