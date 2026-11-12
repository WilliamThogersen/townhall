//! Unit 2.2 conversations: the tables it owns and their store functions (`impl Store` blocks
//! here, using `Store::call`). Migrations 0100 to 0199, files `migrations/01xx_<name>.sql`.
//!
//! `session_dirs`: the folder townhall's own session first ran in. Claude Code keeps a
//! session under its folder, so `--resume` only finds it there: every later turn runs there.

use std::path::{Path, PathBuf};

use rusqlite::{params, OptionalExtension};

use super::codec::path_text;
use super::conversations::get;
use super::migrate::Migration;
use super::Store;
use crate::{clock, Result};

pub const MIGRATIONS: &[Migration] = &[
    Migration {
        id: 100,
        name: "0100_session_dirs",
        sql: include_str!("migrations/0100_session_dirs.sql"),
    },
    Migration {
        id: 101,
        name: "0101_message_reply_to",
        sql: include_str!("migrations/0101_message_reply_to.sql"),
    },
    Migration {
        id: 102,
        name: "0102_turn_resumes",
        sql: include_str!("migrations/0102_turn_resumes.sql"),
    },
    Migration {
        id: 103,
        name: "0103_pending_events",
        sql: include_str!("migrations/0103_pending_events.sql"),
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingEvent {
    pub conversation_id: String,
    pub seq: i64,
    pub text: String,
}

impl Store {
    pub async fn claim_resume(&self, conversation_id: &str, entry_id: u64) -> Result<bool> {
        let (cid, entry) = (conversation_id.to_string(), entry_id as i64);
        self.call(move |conn| {
            get(conn, &cid)?;
            let changed = conn.execute(
                "insert into turn_resumes (conversation_id, entry_id) values (?1, ?2)
                 on conflict (conversation_id) do update set entry_id = excluded.entry_id
                 where turn_resumes.entry_id != excluded.entry_id",
                params![cid, entry],
            )?;
            Ok(changed == 1)
        })
        .await
    }

    pub async fn add_pending_event(
        &self,
        conversation_id: &str,
        seq: i64,
        text: &str,
    ) -> Result<()> {
        let (cid, text) = (conversation_id.to_string(), text.to_string());
        self.call(move |conn| {
            get(conn, &cid)?;
            conn.execute(
                "insert or replace into pending_events (conversation_id, seq, text)
                 values (?1, ?2, ?3)",
                params![cid, seq, text],
            )?;
            Ok(())
        })
        .await
    }

    pub async fn pending_events(&self) -> Result<Vec<PendingEvent>> {
        self.call(|conn| {
            let mut stmt = conn.prepare(
                "select conversation_id, seq, text from pending_events
                 order by conversation_id, seq",
            )?;
            let rows = stmt.query_map([], |r| {
                Ok(PendingEvent {
                    conversation_id: r.get(0)?,
                    seq: r.get(1)?,
                    text: r.get(2)?,
                })
            })?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
        .await
    }

    pub async fn remove_pending_events(&self, conversation_id: &str, seqs: &[i64]) -> Result<()> {
        let (cid, seqs) = (conversation_id.to_string(), seqs.to_vec());
        self.call(move |conn| {
            let tx = conn.transaction()?;
            for seq in seqs {
                tx.execute(
                    "delete from pending_events where conversation_id = ?1 and seq = ?2",
                    params![cid, seq],
                )?;
            }
            tx.commit()?;
            Ok(())
        })
        .await
    }

    /// The folder the conversation's session runs in, once its first turn started.
    pub async fn session_dir(&self, conversation_id: &str) -> Result<Option<PathBuf>> {
        let cid = conversation_id.to_string();
        self.call(move |conn| {
            get(conn, &cid)?;
            let cwd: Option<String> = conn
                .query_row(
                    "select cwd from session_dirs where conversation_id = ?1",
                    [&cid],
                    |r| r.get(0),
                )
                .optional()?;
            Ok(cwd.map(PathBuf::from))
        })
        .await
    }

    /// Records (or replaces) the folder the conversation's session runs in.
    pub async fn set_session_dir(&self, conversation_id: &str, cwd: &Path) -> Result<()> {
        let (cid, cwd) = (conversation_id.to_string(), path_text(cwd)?);
        self.call(move |conn| {
            get(conn, &cid)?;
            conn.execute(
                "insert into session_dirs (conversation_id, cwd, updated_at) values (?1, ?2, ?3)
                 on conflict (conversation_id) do update set cwd = excluded.cwd,
                 updated_at = excluded.updated_at",
                params![cid, cwd, clock::now_rfc3339()],
            )?;
            Ok(())
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use crate::events::EventBus;
    use crate::store::Store;
    use crate::{Error, Home};

    #[tokio::test]
    async fn the_session_dir_is_kept_per_conversation() {
        let dir = tempfile::TempDir::new().unwrap();
        let store = Store::open(&Home::at(dir.path().join(".townhall")), EventBus::new()).unwrap();
        let c = store.create_conversation().await.unwrap();
        assert_eq!(store.session_dir(&c.id).await.unwrap(), None);
        store
            .set_session_dir(&c.id, std::path::Path::new("/r/shop"))
            .await
            .unwrap();
        store
            .set_session_dir(&c.id, std::path::Path::new("/r/admin"))
            .await
            .unwrap();
        assert_eq!(
            store.session_dir(&c.id).await.unwrap(),
            Some("/r/admin".into())
        );
        let missing = store.session_dir("nope").await.unwrap_err();
        assert!(matches!(missing, Error::Missing { .. }), "{missing}");
    }

    async fn open() -> (tempfile::TempDir, Store) {
        let dir = tempfile::TempDir::new().unwrap();
        let store = Store::open(&Home::at(dir.path().join(".townhall")), EventBus::new()).unwrap();
        (dir, store)
    }

    #[tokio::test]
    async fn a_resume_is_claimed_once_per_entry() {
        let (_dir, store) = open().await;
        let c = store.create_conversation().await.unwrap();
        assert!(store.claim_resume(&c.id, 3).await.unwrap());
        assert!(!store.claim_resume(&c.id, 3).await.unwrap());
        assert!(store.claim_resume(&c.id, 5).await.unwrap());
        assert!(!store.claim_resume(&c.id, 5).await.unwrap());
    }

    #[tokio::test]
    async fn pending_events_are_kept_until_removed() {
        let (_dir, store) = open().await;
        let c = store.create_conversation().await.unwrap();
        store.add_pending_event(&c.id, 2, "b").await.unwrap();
        store.add_pending_event(&c.id, 1, "a").await.unwrap();
        let kept = store.pending_events().await.unwrap();
        let texts: Vec<_> = kept.iter().map(|p| (p.seq, p.text.as_str())).collect();
        assert_eq!(texts, [(1, "a"), (2, "b")]);
        store.remove_pending_events(&c.id, &[1]).await.unwrap();
        assert_eq!(store.pending_events().await.unwrap().len(), 1);
    }
}
