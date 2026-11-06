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
];

impl Store {
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
}
