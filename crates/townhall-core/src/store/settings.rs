//! The `settings` table: one row per key, the value as JSON.

use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{Map, Value};

use super::codec::path_text;
use super::Store;
use crate::model::{SettingKey, Settings};
use crate::{clock, Error, Result};

/// The desktop app's chosen repo (its startup repo). Not a user setting, not in the API.
pub(super) const DESKTOP_REPO: &str = "desktop.repo";

/// The user's profile text (`GET/PUT/DELETE /api/profile`). Not a setting: no `SettingKey`,
/// so `PUT /api/settings/{key}` cannot reach it. It stays in this database.
const PROFILE: &str = "profile.text";

/// The longest profile, in characters. It goes into the session's system prompt.
pub const MAX_PROFILE_CHARS: usize = 20_000;

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
            Err(e) => tracing::warn!(
                target: "townhall::store", event = "store.setting_ignored", key = %key,
                error = %crate::redact::redact(&e.to_string()), "setting ignored"
            ),
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

    /// The profile text; empty when none was saved.
    pub async fn profile(&self) -> Result<String> {
        self.call(|conn| {
            let text: Option<String> = conn
                .query_row(
                    "select value from settings where key = ?1",
                    [PROFILE],
                    |r| r.get(0),
                )
                .optional()?;
            Ok(text
                .and_then(|t| serde_json::from_str::<String>(&t).ok())
                .unwrap_or_default())
        })
        .await
    }

    /// Replaces the profile; answers with what is stored.
    pub async fn set_profile(&self, text: &str) -> Result<String> {
        if text.chars().count() > MAX_PROFILE_CHARS {
            return Err(Error::InvalidInput(format!(
                "the profile is at most {MAX_PROFILE_CHARS} characters"
            )));
        }
        let value = Value::String(text.to_string());
        self.call(move |conn| put(conn, PROFILE, &value)).await?;
        Ok(text.to_string())
    }

    /// Resets the profile to empty; deleting an empty one is fine.
    pub async fn delete_profile(&self) -> Result<()> {
        self.call(|conn| {
            conn.execute("delete from settings where key = ?1", [PROFILE])?;
            Ok(())
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::events::EventBus;
    use crate::model::SettingKey;
    use crate::store::Store;
    use crate::Home;

    #[tokio::test]
    async fn a_new_database_expects_the_owners_github_account() {
        let dir = tempfile::TempDir::new().unwrap();
        let store = Store::open(&Home::at(dir.path().join(".townhall")), EventBus::new()).unwrap();
        assert_eq!(
            store.settings().await.unwrap().github_account,
            "WilliamThogersen"
        );
        let cleared = store
            .set_setting(SettingKey::GithubAccount, json!(""))
            .await
            .unwrap();
        assert_eq!(cleared.github_account, "");
    }
}
