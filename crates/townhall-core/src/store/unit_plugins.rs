use std::collections::BTreeMap;

use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;

use super::codec::{from_json, to_json};
use super::migrate::Migration;
use super::Store;
use crate::{clock, Error, Result};

pub const MIGRATIONS: &[Migration] = &[
    Migration {
        id: 950,
        name: "0950_plugins",
        sql: include_str!("migrations/0950_plugins.sql"),
    },
    Migration {
        id: 951,
        name: "0951_plugin_settings",
        sql: include_str!("migrations/0951_plugin_settings.sql"),
    },
];

const THEME: &str = "plugins.theme";

#[derive(Debug, Clone, PartialEq)]
pub struct Marketplace {
    pub name: String,
    pub source: Value,
    pub account: Option<String>,
    pub enabled: bool,
    pub official: bool,
    pub head_sha: Option<String>,
    pub fetched_at: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Plugin {
    pub id: String,
    pub marketplace: String,
    pub name: String,
    pub version: Option<String>,
    pub sha: String,
    pub source: Value,
    pub enabled: bool,
    pub pinned: bool,
    pub permissions_hash: String,
    pub approved: Value,
    pub installed_at: String,
    pub updated_at: String,
    pub error: Option<String>,
}

const MARKETPLACE_COLUMNS: &str =
    "name, source, account, enabled, official, head_sha, fetched_at, error";

const PLUGIN_COLUMNS: &str = "id, marketplace, name, version, sha, source, enabled, pinned, \
     permissions_hash, approved, installed_at, updated_at, error";

type MarketplaceRow = (
    String,
    String,
    Option<String>,
    bool,
    bool,
    Option<String>,
    Option<String>,
    Option<String>,
);

type PluginRow = (
    String,
    String,
    String,
    Option<String>,
    String,
    String,
    bool,
    bool,
    String,
    String,
    String,
    String,
    Option<String>,
);

fn marketplace_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<MarketplaceRow> {
    Ok((
        r.get(0)?,
        r.get(1)?,
        r.get(2)?,
        r.get(3)?,
        r.get(4)?,
        r.get(5)?,
        r.get(6)?,
        r.get(7)?,
    ))
}

fn plugin_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<PluginRow> {
    Ok((
        r.get(0)?,
        r.get(1)?,
        r.get(2)?,
        r.get(3)?,
        r.get(4)?,
        r.get(5)?,
        r.get(6)?,
        r.get(7)?,
        r.get(8)?,
        r.get(9)?,
        r.get(10)?,
        r.get(11)?,
        r.get(12)?,
    ))
}

fn to_marketplace(row: MarketplaceRow) -> Result<Marketplace> {
    let (name, source, account, enabled, official, head_sha, fetched_at, error) = row;
    Ok(Marketplace {
        source: from_json("marketplaces.source", &source)?,
        name,
        account,
        enabled,
        official,
        head_sha,
        fetched_at,
        error,
    })
}

fn to_plugin(row: PluginRow) -> Result<Plugin> {
    let (id, marketplace, name, version, sha, source, enabled, pinned) =
        (row.0, row.1, row.2, row.3, row.4, row.5, row.6, row.7);
    let (permissions_hash, approved, installed_at, updated_at, error) =
        (row.8, row.9, row.10, row.11, row.12);
    Ok(Plugin {
        source: from_json("plugins.source", &source)?,
        approved: from_json("plugins.approved", &approved)?,
        id,
        marketplace,
        name,
        version,
        sha,
        enabled,
        pinned,
        permissions_hash,
        installed_at,
        updated_at,
        error,
    })
}

fn find_marketplace(conn: &Connection, name: &str) -> Result<Option<Marketplace>> {
    let sql = format!("select {MARKETPLACE_COLUMNS} from marketplaces where name = ?1");
    let row = conn.query_row(&sql, [name], marketplace_row).optional()?;
    row.map(to_marketplace).transpose()
}

fn find_plugin(conn: &Connection, id: &str) -> Result<Option<Plugin>> {
    let sql = format!("select {PLUGIN_COLUMNS} from plugins where id = ?1");
    let row = conn.query_row(&sql, [id], plugin_row).optional()?;
    row.map(to_plugin).transpose()
}

fn list_marketplaces(conn: &Connection) -> Result<Vec<Marketplace>> {
    let sql = format!("select {MARKETPLACE_COLUMNS} from marketplaces order by rowid");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], marketplace_row)?;
    rows.map(|row| to_marketplace(row?)).collect()
}

fn list_plugins(conn: &Connection) -> Result<Vec<Plugin>> {
    let sql = format!("select {PLUGIN_COLUMNS} from plugins order by installed_at, rowid");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], plugin_row)?;
    rows.map(|row| to_plugin(row?)).collect()
}

fn insert_marketplace(conn: &Connection, m: &Marketplace) -> Result<()> {
    if find_marketplace(conn, &m.name)?.is_some() {
        return Err(Error::Conflict(format!(
            "a marketplace named `{}` already exists",
            m.name
        )));
    }
    conn.execute(
        "insert into marketplaces (name, source, account, enabled, official, head_sha, fetched_at, error)
         values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            m.name,
            to_json(&m.source)?,
            m.account,
            m.enabled,
            m.official,
            m.head_sha,
            m.fetched_at,
            m.error
        ],
    )?;
    Ok(())
}

fn save_marketplace(conn: &Connection, m: &Marketplace) -> Result<Marketplace> {
    let found =
        find_marketplace(conn, &m.name)?.ok_or_else(|| Error::missing("marketplace", &m.name))?;
    conn.execute(
        "update marketplaces set source = ?1, account = ?2, enabled = ?3, official = ?4,
         head_sha = ?5, fetched_at = ?6, error = ?7 where name = ?8",
        params![
            to_json(&m.source)?,
            m.account,
            m.enabled,
            m.official,
            m.head_sha,
            m.fetched_at,
            m.error,
            found.name
        ],
    )?;
    find_marketplace(conn, &found.name)?.ok_or_else(|| Error::missing("marketplace", &found.name))
}

fn delete_marketplace(conn: &Connection, name: &str) -> Result<Marketplace> {
    let found = find_marketplace(conn, name)?.ok_or_else(|| Error::missing("marketplace", name))?;
    let held: Vec<String> = list_plugins(conn)?
        .into_iter()
        .filter(|p| p.marketplace.eq_ignore_ascii_case(&found.name))
        .map(|p| p.id)
        .collect();
    if !held.is_empty() {
        return Err(Error::Conflict(format!(
            "marketplace `{}` still has installed plugins: {}",
            found.name,
            held.join(", ")
        )));
    }
    conn.execute("delete from marketplaces where name = ?1", [&found.name])?;
    Ok(found)
}

fn insert_plugin(conn: &Connection, p: &Plugin) -> Result<Plugin> {
    let market = find_marketplace(conn, &p.marketplace)?
        .ok_or_else(|| Error::missing("marketplace", &p.marketplace))?;
    let id = format!("{}@{}", p.name, market.name);
    if find_plugin(conn, &id)?.is_some() {
        return Err(Error::Conflict(format!(
            "plugin `{id}` is already installed"
        )));
    }
    let now = clock::now_rfc3339();
    conn.execute(
        "insert into plugins (id, marketplace, name, version, sha, source, enabled, pinned,
         permissions_hash, approved, installed_at, updated_at, error)
         values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11, ?12)",
        params![
            id,
            market.name,
            p.name,
            p.version,
            p.sha,
            to_json(&p.source)?,
            p.enabled,
            p.pinned,
            p.permissions_hash,
            to_json(&p.approved)?,
            now,
            p.error
        ],
    )?;
    find_plugin(conn, &id)?.ok_or_else(|| Error::missing("plugin", &id))
}

fn save_plugin(conn: &Connection, p: &Plugin) -> Result<Plugin> {
    let found = find_plugin(conn, &p.id)?.ok_or_else(|| Error::missing("plugin", &p.id))?;
    conn.execute(
        "update plugins set version = ?1, sha = ?2, source = ?3, enabled = ?4, pinned = ?5,
         permissions_hash = ?6, approved = ?7, error = ?8, updated_at = ?9 where id = ?10",
        params![
            p.version,
            p.sha,
            to_json(&p.source)?,
            p.enabled,
            p.pinned,
            p.permissions_hash,
            to_json(&p.approved)?,
            p.error,
            clock::now_rfc3339(),
            found.id
        ],
    )?;
    find_plugin(conn, &found.id)?.ok_or_else(|| Error::missing("plugin", &found.id))
}

fn delete_plugin(conn: &Connection, id: &str) -> Result<Plugin> {
    let found = find_plugin(conn, id)?.ok_or_else(|| Error::missing("plugin", id))?;
    conn.execute("delete from plugins where id = ?1", [&found.id])?;
    Ok(found)
}

impl Store {
    pub async fn marketplaces(&self) -> Result<Vec<Marketplace>> {
        self.call(|conn| list_marketplaces(conn)).await
    }

    pub async fn marketplace(&self, name: &str) -> Result<Marketplace> {
        let name = name.to_string();
        self.call(move |conn| {
            find_marketplace(conn, &name)?.ok_or_else(|| Error::missing("marketplace", &name))
        })
        .await
    }

    pub async fn add_marketplace(&self, marketplace: Marketplace) -> Result<Marketplace> {
        self.call(move |conn| {
            let tx = conn.transaction()?;
            insert_marketplace(&tx, &marketplace)?;
            let saved = find_marketplace(&tx, &marketplace.name)?
                .ok_or_else(|| Error::missing("marketplace", &marketplace.name))?;
            tx.commit()?;
            Ok(saved)
        })
        .await
    }

    pub async fn update_marketplace(&self, marketplace: Marketplace) -> Result<Marketplace> {
        self.call(move |conn| save_marketplace(conn, &marketplace))
            .await
    }

    pub async fn remove_marketplace(&self, name: &str) -> Result<Marketplace> {
        let name = name.to_string();
        self.call(move |conn| {
            let tx = conn.transaction()?;
            let removed = delete_marketplace(&tx, &name)?;
            tx.commit()?;
            Ok(removed)
        })
        .await
    }

    pub async fn plugins(&self) -> Result<Vec<Plugin>> {
        self.call(|conn| list_plugins(conn)).await
    }

    pub async fn plugin(&self, id: &str) -> Result<Plugin> {
        let id = id.to_string();
        self.call(move |conn| find_plugin(conn, &id)?.ok_or_else(|| Error::missing("plugin", &id)))
            .await
    }

    pub async fn add_plugin(&self, plugin: Plugin) -> Result<Plugin> {
        self.call(move |conn| {
            let tx = conn.transaction()?;
            let saved = insert_plugin(&tx, &plugin)?;
            tx.commit()?;
            Ok(saved)
        })
        .await
    }

    pub async fn update_plugin(&self, plugin: Plugin) -> Result<Plugin> {
        self.call(move |conn| save_plugin(conn, &plugin)).await
    }

    pub async fn remove_plugin(&self, id: &str) -> Result<Plugin> {
        let id = id.to_string();
        self.call(move |conn| delete_plugin(conn, &id)).await
    }

    pub async fn plugin_settings(&self, id: &str) -> Result<BTreeMap<String, Value>> {
        let id = id.to_string();
        self.call(move |conn| {
            let mut stmt = conn.prepare(
                "select key, value from plugin_settings where plugin_id = ?1 order by key",
            )?;
            let rows = stmt.query_map([&id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?;
            let mut found = BTreeMap::new();
            for row in rows {
                let (key, text) = row?;
                found.insert(key, from_json("plugin_settings.value", &text)?);
            }
            Ok(found)
        })
        .await
    }

    pub async fn set_plugin_setting(&self, id: &str, key: &str, value: Value) -> Result<()> {
        let (id, key) = (id.to_string(), key.to_string());
        self.call(move |conn| {
            conn.execute(
                "insert into plugin_settings (plugin_id, key, value, updated_at)
                 values (?1, ?2, ?3, ?4)
                 on conflict (plugin_id, key) do update
                 set value = excluded.value, updated_at = excluded.updated_at",
                params![id, key, to_json(&value)?, clock::now_rfc3339()],
            )?;
            Ok(())
        })
        .await
    }

    pub async fn reset_plugin_setting(&self, id: &str, key: &str) -> Result<()> {
        let (id, key) = (id.to_string(), key.to_string());
        self.call(move |conn| {
            conn.execute(
                "delete from plugin_settings where plugin_id = ?1 and key = ?2",
                params![id, key],
            )?;
            Ok(())
        })
        .await
    }

    pub async fn clear_plugin_settings(&self, id: &str) -> Result<()> {
        let id = id.to_string();
        self.call(move |conn| {
            conn.execute("delete from plugin_settings where plugin_id = ?1", [&id])?;
            Ok(())
        })
        .await
    }

    pub async fn plugin_theme(&self) -> Result<Option<String>> {
        self.call(|conn| {
            let text: Option<String> = conn
                .query_row("select value from settings where key = ?1", [THEME], |r| {
                    r.get(0)
                })
                .optional()?;
            Ok(text.and_then(|t| serde_json::from_str::<String>(&t).ok()))
        })
        .await
    }

    pub async fn set_plugin_theme(&self, id: Option<&str>) -> Result<()> {
        let id = id.map(str::to_string);
        self.call(move |conn| match id {
            Some(id) => super::settings::put(conn, THEME, &Value::String(id)),
            None => {
                conn.execute("delete from settings where key = ?1", [THEME])?;
                Ok(())
            }
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::events::EventBus;
    use crate::store::unit_plugins::{Marketplace, Plugin};
    use crate::store::Store;
    use crate::{Error, Home};

    fn open() -> (tempfile::TempDir, Store) {
        let dir = tempfile::TempDir::new().unwrap();
        let store = Store::open(&Home::at(dir.path().join(".townhall")), EventBus::new()).unwrap();
        (dir, store)
    }

    fn marketplace(name: &str) -> Marketplace {
        Marketplace {
            name: name.into(),
            source: json!({"source": "github", "repo": "acme/market"}),
            account: Some("work-me".into()),
            enabled: true,
            official: false,
            head_sha: Some("abc123".into()),
            fetched_at: Some("2026-11-09T10:00:00Z".into()),
            error: None,
        }
    }

    fn plugin(name: &str, marketplace: &str) -> Plugin {
        Plugin {
            id: String::new(),
            marketplace: marketplace.into(),
            name: name.into(),
            version: Some("1.2.0".into()),
            sha: "deadbeef".into(),
            source: json!({"source": "git-subdir", "path": "plugins/x"}),
            enabled: false,
            pinned: false,
            permissions_hash: "sha256:aa".into(),
            approved: json!({"api": ["tasks:read"]}),
            installed_at: String::new(),
            updated_at: String::new(),
            error: None,
        }
    }

    #[tokio::test]
    async fn a_marketplace_round_trips_through_add_read_list_update_and_remove() {
        let (_dir, store) = open();
        let added = store.add_marketplace(marketplace("acme")).await.unwrap();
        assert_eq!(added, marketplace("acme"));
        assert_eq!(store.marketplace("acme").await.unwrap(), added);
        store.add_marketplace(marketplace("beta")).await.unwrap();
        let names: Vec<String> = store
            .marketplaces()
            .await
            .unwrap()
            .into_iter()
            .map(|m| m.name)
            .collect();
        assert_eq!(names, ["acme", "beta"]);

        let next = Marketplace {
            account: None,
            enabled: false,
            head_sha: Some("fff000".into()),
            fetched_at: Some("2026-11-10T08:00:00Z".into()),
            error: Some("offline".into()),
            ..marketplace("acme")
        };
        let saved = store.update_marketplace(next.clone()).await.unwrap();
        assert_eq!(saved, next);
        assert_eq!(store.marketplace("acme").await.unwrap(), next);

        let removed = store.remove_marketplace("acme").await.unwrap();
        assert_eq!(removed, next);
        let err = store.marketplace("acme").await.unwrap_err();
        assert!(matches!(err, Error::Missing { .. }), "{err}");
        assert_eq!(store.marketplaces().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn marketplace_names_are_unique_and_found_ignoring_case() {
        let (_dir, store) = open();
        store.add_marketplace(marketplace("Acme")).await.unwrap();
        let err = store
            .add_marketplace(marketplace("ACME"))
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Conflict(_)), "{err}");
        assert_eq!(store.marketplace("aCmE").await.unwrap().name, "Acme");
        let moved = Marketplace {
            enabled: false,
            ..marketplace("acme")
        };
        assert_eq!(store.update_marketplace(moved).await.unwrap().name, "Acme");
        assert_eq!(store.remove_marketplace("ACME").await.unwrap().name, "Acme");
        let missing = store.update_marketplace(marketplace("ghost")).await;
        assert!(matches!(missing, Err(Error::Missing { .. })));
        let missing = store.remove_marketplace("ghost").await;
        assert!(matches!(missing, Err(Error::Missing { .. })));
    }

    #[tokio::test]
    async fn a_plugin_round_trips_through_add_read_list_update_and_remove() {
        let (_dir, store) = open();
        store.add_marketplace(marketplace("acme")).await.unwrap();
        let added = store.add_plugin(plugin("lint", "acme")).await.unwrap();
        assert_eq!(added.id, "lint@acme");
        assert!(!added.installed_at.is_empty());
        assert_eq!(added.installed_at, added.updated_at);
        assert_eq!(
            Plugin {
                id: String::new(),
                installed_at: String::new(),
                updated_at: String::new(),
                ..added.clone()
            },
            plugin("lint", "acme")
        );
        assert_eq!(store.plugin("lint@acme").await.unwrap(), added);
        store.add_plugin(plugin("fmt", "acme")).await.unwrap();
        let ids: Vec<String> = store
            .plugins()
            .await
            .unwrap()
            .into_iter()
            .map(|p| p.id)
            .collect();
        assert_eq!(ids, ["lint@acme", "fmt@acme"]);

        let next = Plugin {
            version: Some("1.3.0".into()),
            sha: "cafe".into(),
            enabled: true,
            pinned: true,
            approved: json!({"api": ["tasks:read", "storage"]}),
            error: Some("hook failed".into()),
            ..added.clone()
        };
        let saved = store.update_plugin(next.clone()).await.unwrap();
        assert_eq!(saved.installed_at, added.installed_at);
        assert_eq!(
            Plugin {
                updated_at: added.updated_at.clone(),
                ..saved.clone()
            },
            next
        );
        assert_eq!(store.plugin("lint@acme").await.unwrap(), saved);

        let removed = store.remove_plugin("lint@acme").await.unwrap();
        assert_eq!(removed, saved);
        let err = store.plugin("lint@acme").await.unwrap_err();
        assert!(matches!(err, Error::Missing { .. }), "{err}");
        assert_eq!(store.plugins().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn plugin_ids_are_unique_and_found_ignoring_case() {
        let (_dir, store) = open();
        store.add_marketplace(marketplace("Acme")).await.unwrap();
        let added = store.add_plugin(plugin("Lint", "acme")).await.unwrap();
        assert_eq!(added.id, "Lint@Acme");
        assert_eq!(added.marketplace, "Acme");
        let err = store.add_plugin(plugin("LINT", "ACME")).await.unwrap_err();
        assert!(matches!(err, Error::Conflict(_)), "{err}");
        assert_eq!(store.plugin("lint@ACME").await.unwrap().id, "Lint@Acme");
        let renamed = Plugin {
            id: "LINT@acme".into(),
            enabled: true,
            ..added
        };
        assert_eq!(store.update_plugin(renamed).await.unwrap().id, "Lint@Acme");
        assert_eq!(
            store.remove_plugin("LINT@ACME").await.unwrap().id,
            "Lint@Acme"
        );
    }

    #[tokio::test]
    async fn a_plugin_needs_its_marketplace_and_a_marketplace_with_plugins_stays() {
        let (_dir, store) = open();
        let err = store.add_plugin(plugin("lint", "acme")).await.unwrap_err();
        assert!(matches!(err, Error::Missing { .. }), "{err}");
        store.add_marketplace(marketplace("acme")).await.unwrap();
        store.add_plugin(plugin("lint", "acme")).await.unwrap();
        let err = store.remove_marketplace("acme").await.unwrap_err();
        assert!(matches!(err, Error::Conflict(_)), "{err}");
        assert!(err.to_string().contains("lint@acme"), "{err}");
        store.remove_plugin("lint@acme").await.unwrap();
        store.remove_marketplace("acme").await.unwrap();
    }

    #[tokio::test]
    async fn plugin_settings_round_trip_per_plugin_and_reset_to_nothing() {
        let (_dir, store) = open();
        assert!(store.plugin_settings("lint@acme").await.unwrap().is_empty());
        store
            .set_plugin_setting("lint@acme", "retries", json!(3))
            .await
            .unwrap();
        store
            .set_plugin_setting("lint@acme", "mode", json!("fast"))
            .await
            .unwrap();
        store
            .set_plugin_setting("fmt@acme", "retries", json!(9))
            .await
            .unwrap();
        let lint = store.plugin_settings("lint@acme").await.unwrap();
        assert_eq!(lint.len(), 2);
        assert_eq!(lint["retries"], json!(3));
        assert_eq!(lint["mode"], json!("fast"));
        assert_eq!(
            store.plugin_settings("fmt@acme").await.unwrap()["retries"],
            json!(9)
        );

        store
            .set_plugin_setting("lint@acme", "retries", json!(4))
            .await
            .unwrap();
        assert_eq!(
            store.plugin_settings("LINT@ACME").await.unwrap()["retries"],
            json!(4)
        );

        store
            .reset_plugin_setting("lint@acme", "retries")
            .await
            .unwrap();
        store
            .reset_plugin_setting("lint@acme", "never-set")
            .await
            .unwrap();
        assert_eq!(store.plugin_settings("lint@acme").await.unwrap().len(), 1);

        store.clear_plugin_settings("lint@acme").await.unwrap();
        assert!(store.plugin_settings("lint@acme").await.unwrap().is_empty());
        assert_eq!(store.plugin_settings("fmt@acme").await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn plugin_settings_outlive_the_plugin_row_and_the_store() {
        let dir = tempfile::TempDir::new().unwrap();
        let home = Home::at(dir.path().join(".townhall"));
        {
            let store = Store::open(&home, EventBus::new()).unwrap();
            store
                .set_plugin_setting("gone@acme", "k", json!({"a": [1, 2]}))
                .await
                .unwrap();
        }
        let store = Store::open(&home, EventBus::new()).unwrap();
        assert_eq!(
            store.plugin_settings("gone@acme").await.unwrap()["k"],
            json!({"a": [1, 2]})
        );
    }

    #[tokio::test]
    async fn the_active_theme_is_one_row_outside_the_settings_keys() {
        let (_dir, store) = open();
        let before = store.settings().await.unwrap();
        assert_eq!(store.plugin_theme().await.unwrap(), None);
        store.set_plugin_theme(Some("slate@acme")).await.unwrap();
        assert_eq!(
            store.plugin_theme().await.unwrap().as_deref(),
            Some("slate@acme")
        );
        store.set_plugin_theme(Some("other@acme")).await.unwrap();
        assert_eq!(
            store.plugin_theme().await.unwrap().as_deref(),
            Some("other@acme")
        );
        assert_eq!(store.settings().await.unwrap(), before);
        store.set_plugin_theme(None).await.unwrap();
        assert_eq!(store.plugin_theme().await.unwrap(), None);
        store.set_plugin_theme(None).await.unwrap();
    }

    #[tokio::test]
    async fn plugins_and_marketplaces_survive_reopening_the_store() {
        let dir = tempfile::TempDir::new().unwrap();
        let home = Home::at(dir.path().join(".townhall"));
        {
            let store = Store::open(&home, EventBus::new()).unwrap();
            store.add_marketplace(marketplace("acme")).await.unwrap();
            store.add_plugin(plugin("lint", "acme")).await.unwrap();
        }
        let store = Store::open(&home, EventBus::new()).unwrap();
        assert_eq!(store.marketplaces().await.unwrap().len(), 1);
        assert_eq!(store.plugins().await.unwrap().len(), 1);
    }
}
