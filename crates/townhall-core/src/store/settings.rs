//! The `settings` table: one row per key, the value as JSON.

use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{Map, Value};

use super::codec::path_text;
use super::Store;
use crate::model::{SettingKey, Settings};
use crate::{clock, Result};

/// The desktop app's chosen repo (its startup repo). Not a user setting, not in the API.
pub(super) const DESKTOP_REPO: &str = "desktop.repo";

fn rows(conn: &Connection) -> Result<Map<String, Value>> {
    let mut stmt = conn.prepare("select key, value from settings")?;
    let pairs = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    let mut map = Map::new();
    for pair in pairs {
        let (key, text) = pair?;
        match serde_json::from_str(&text) {
            Ok(value) => {
                map.insert(key, value);
            }
            Err(e) => eprintln!("warn: setting `{key}` ignored: {e}"),
        }
    }
    Ok(map)
}

pub(super) fn read(conn: &Connection) -> Result<Settings> {
    Ok(Settings::from_map(rows(conn)?))
}

pub(super) fn put(conn: &Connection, key: &str, value: &Value) -> Result<()> {
    conn.execute(
        "insert into settings (key, value, updated_at) values (?1, ?2, ?3)
         on conflict (key) do update set value = excluded.value, updated_at = excluded.updated_at",
        params![key, value.to_string(), clock::now_rfc3339()],
    )?;
    Ok(())
}

/// Writes the row only when the key has none (the import never overrides).
pub(super) fn put_missing(conn: &Connection, key: &str, value: &Value) -> Result<()> {
    conn.execute(
        "insert into settings (key, value, updated_at) values (?1, ?2, ?3)
         on conflict (key) do nothing",
        params![key, value.to_string(), clock::now_rfc3339()],
    )?;
    Ok(())
}

impl Store {
    /// Missing or invalid keys read as their defaults.
    pub async fn settings(&self) -> Result<Settings> {
        self.call(|conn| read(conn)).await
    }

    /// `PUT /api/settings/{key}`, the only way to change a setting, and so the only way
    /// to choose `skip all`. Validates the value for the key.
    pub async fn set_setting(&self, key: SettingKey, value: Value) -> Result<Settings> {
        let value = key.check(&value)?;
        self.call(move |conn| {
            put(conn, key.as_str(), &value)?;
            read(conn)
        })
        .await
    }

    pub async fn desktop_repo(&self) -> Result<Option<PathBuf>> {
        self.call(|conn| {
            let text: Option<String> = conn
                .query_row(
                    "select value from settings where key = ?1",
                    [DESKTOP_REPO],
                    |r| r.get(0),
                )
                .optional()?;
            Ok(text
                .and_then(|t| serde_json::from_str::<String>(&t).ok())
                .map(PathBuf::from))
        })
        .await
    }

    pub async fn set_desktop_repo(&self, path: &Path) -> Result<()> {
        let value = Value::String(path_text(path)?);
        self.call(move |conn| put(conn, DESKTOP_REPO, &value)).await
    }
}
