use rusqlite::{params, OptionalExtension};

use super::Store;
use crate::{clock, Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenRow {
    pub token_hash: String,
    pub plugin_id: String,
    pub task: Option<String>,
    pub created_at: String,
    pub revoked_at: Option<String>,
}

fn token_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<TokenRow> {
    Ok(TokenRow {
        token_hash: r.get(0)?,
        plugin_id: r.get(1)?,
        task: r.get(2)?,
        created_at: r.get(3)?,
        revoked_at: r.get(4)?,
    })
}

const TOKEN_COLUMNS: &str = "token_hash, plugin_id, task, created_at, revoked_at";

impl Store {
    pub async fn add_plugin_token(
        &self,
        plugin_id: &str,
        task: Option<&str>,
        token_hash: &str,
    ) -> Result<()> {
        let (plugin_id, task, token_hash) = (
            plugin_id.to_string(),
            task.map(str::to_string),
            token_hash.to_string(),
        );
        self.call(move |conn| {
            conn.execute(
                "insert into plugin_tokens (token_hash, plugin_id, task, created_at)
                 values (?1, ?2, ?3, ?4)",
                params![token_hash, plugin_id, task, clock::now_rfc3339()],
            )?;
            Ok(())
        })
        .await
    }

    pub async fn plugin_tokens(&self, plugin_id: &str) -> Result<Vec<TokenRow>> {
        let plugin_id = plugin_id.to_string();
        self.call(move |conn| {
            let sql = format!(
                "select {TOKEN_COLUMNS} from plugin_tokens where plugin_id = ?1 order by rowid"
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map([&plugin_id], token_row)?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
        .await
    }

    pub async fn live_plugin_token(&self, token_hash: &str) -> Result<Option<TokenRow>> {
        let token_hash = token_hash.to_string();
        self.call(move |conn| {
            let sql = format!(
                "select {TOKEN_COLUMNS} from plugin_tokens
                 where token_hash = ?1 and revoked_at is null"
            );
            Ok(conn.query_row(&sql, [&token_hash], token_row).optional()?)
        })
        .await
    }

    pub async fn revoke_plugin_tokens(&self, plugin_id: &str) -> Result<()> {
        let plugin_id = plugin_id.to_string();
        self.call(move |conn| {
            conn.execute(
                "update plugin_tokens set revoked_at = ?1
                 where plugin_id = ?2 and revoked_at is null",
                params![clock::now_rfc3339(), plugin_id],
            )?;
            Ok(())
        })
        .await
    }

    pub async fn revoke_task_tokens(&self, task: &str) -> Result<()> {
        let task = task.to_string();
        self.call(move |conn| {
            conn.execute(
                "update plugin_tokens set revoked_at = ?1
                 where task = ?2 and revoked_at is null",
                params![clock::now_rfc3339(), task],
            )?;
            Ok(())
        })
        .await
    }

    pub async fn plugin_storage_get(&self, plugin_id: &str, key: &str) -> Result<Option<String>> {
        let (plugin_id, key) = (plugin_id.to_string(), key.to_string());
        self.call(move |conn| {
            Ok(conn
                .query_row(
                    "select value from plugin_storage where plugin_id = ?1 and key = ?2",
                    params![plugin_id, key],
                    |r| r.get(0),
                )
                .optional()?)
        })
        .await
    }

    pub async fn plugin_storage_put(
        &self,
        plugin_id: &str,
        key: &str,
        value: &str,
        total_cap: usize,
    ) -> Result<()> {
        let (plugin_id, key, value) = (plugin_id.to_string(), key.to_string(), value.to_string());
        self.call(move |conn| {
            let tx = conn.transaction()?;
            let others: i64 = tx.query_row(
                "select coalesce(sum(length(cast(value as blob))), 0) from plugin_storage
                 where plugin_id = ?1 and key <> ?2",
                params![plugin_id, key],
                |r| r.get(0),
            )?;
            if others as usize + value.len() > total_cap {
                return Err(Error::TooLarge(format!(
                    "plugin `{plugin_id}` would store more than {total_cap} bytes in all"
                )));
            }
            tx.execute(
                "insert into plugin_storage (plugin_id, key, value, updated_at)
                 values (?1, ?2, ?3, ?4)
                 on conflict (plugin_id, key) do update
                 set value = excluded.value, updated_at = excluded.updated_at",
                params![plugin_id, key, value, clock::now_rfc3339()],
            )?;
            tx.commit()?;
            Ok(())
        })
        .await
    }

    pub async fn plugin_storage_delete(&self, plugin_id: &str, key: &str) -> Result<bool> {
        let (plugin_id, key) = (plugin_id.to_string(), key.to_string());
        self.call(move |conn| {
            let gone = conn.execute(
                "delete from plugin_storage where plugin_id = ?1 and key = ?2",
                params![plugin_id, key],
            )?;
            Ok(gone > 0)
        })
        .await
    }

    pub async fn clear_plugin_storage(&self, plugin_id: &str) -> Result<()> {
        let plugin_id = plugin_id.to_string();
        self.call(move |conn| {
            conn.execute(
                "delete from plugin_storage where plugin_id = ?1",
                [&plugin_id],
            )?;
            Ok(())
        })
        .await
    }
}
