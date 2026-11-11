use std::fs;

use rusqlite::Connection;
use serde_json::json;
use tempfile::TempDir;
use townhall_core::events::EventBus;
use townhall_core::model::{SettingKey, Settings};
use townhall_core::store::{Store, DB_FILE, LOCK_FILE};
use townhall_core::{Error, Home};

mod common;
use common::init_named;

fn home(dir: &TempDir) -> Home {
    Home::at(dir.path().join(".townhall"))
}

fn open(dir: &TempDir) -> Store {
    Store::open(&home(dir), EventBus::new()).unwrap()
}

fn write_config(dir: &TempDir, value: serde_json::Value) {
    let h = home(dir);
    h.ensure().unwrap();
    fs::write(h.config_file(), value.to_string()).unwrap();
}

fn seeded() -> Settings {
    Settings {
        github_account: "WilliamThogersen".into(),
        ..Settings::default()
    }
}

/// A database migrated up to the seed of `default` (0806), as an existing install has it,
/// with the `default` row's source set to `source` (or the row deleted for `None`).
fn at_0806(source: Option<&str>) -> Connection {
    let mut conn = Connection::open_in_memory().unwrap();
    let before: Vec<_> = townhall_core::store::migrate::all()
        .into_iter()
        .filter(|m| m.id <= 806)
        .collect();
    townhall_core::store::migrate::apply(&mut conn, &before).unwrap();
    match source {
        Some(source) => conn
            .execute(
                "update agents set source = ?1 where name = 'default'",
                [source],
            )
            .unwrap(),
        None => conn
            .execute("delete from agents where name = 'default'", [])
            .unwrap(),
    };
    conn
}

fn upgrade(conn: &mut Connection) {
    townhall_core::store::migrate::apply(conn, &townhall_core::store::migrate::all()).unwrap();
}

fn default_source(conn: &Connection) -> Option<String> {
    conn.query_row(
        "select source from agents where name = 'default'",
        [],
        |r| r.get(0),
    )
    .ok()
}

#[test]
fn the_default_type_with_no_model_gets_opus() {
    let mut conn = at_0806(Some(
        "name = \"default\"\nadapter = \"claude\"\nprompt = \"hi\"\n",
    ));
    upgrade(&mut conn);
    let source = default_source(&conn).unwrap();
    assert!(source.lines().any(|l| l == "model = \"opus\""), "{source}");
    assert!(source.contains("prompt = \"hi\""), "{source}");
}

#[test]
fn a_default_type_that_has_a_model_is_left_alone() {
    let own = "name = \"default\"\nadapter = \"claude\"\nmodel = \"haiku\"\n";
    let mut conn = at_0806(Some(own));
    upgrade(&mut conn);
    assert_eq!(default_source(&conn).unwrap(), own);
}

#[test]
fn the_seeded_default_type_gains_the_web_tools() {
    let mut old = Connection::open_in_memory().unwrap();
    let before: Vec<_> = townhall_core::store::migrate::all()
        .into_iter()
        .filter(|m| m.id <= 807)
        .collect();
    townhall_core::store::migrate::apply(&mut old, &before).unwrap();
    let seeded = default_source(&old).unwrap();
    assert!(!seeded.contains("WebFetch"), "{seeded}");
    upgrade(&mut old);
    let source = default_source(&old).unwrap();
    assert!(source.contains("  \"Glob\",\n  \"WebFetch\",\n  \"WebSearch\"\n]"));
    assert!(source.contains("the web (WebFetch, WebSearch) only to plan or answer"));
    assert!(!source.contains("to plan, nothing else"));
}

#[test]
fn a_default_type_with_its_own_tools_and_prompt_is_left_alone() {
    let own = "name = \"default\"\nadapter = \"claude\"\nmodel = \"haiku\"\ntools = [\"Read\"]\nprompt = \"hi\"\n";
    let mut conn = at_0806(Some(own));
    upgrade(&mut conn);
    assert_eq!(default_source(&conn).unwrap(), own);
}

#[test]
fn a_missing_default_type_is_not_created() {
    let mut conn = at_0806(None);
    upgrade(&mut conn);
    assert_eq!(default_source(&conn), None);
}

#[test]
fn a_fresh_database_seeds_the_default_type_with_opus() {
    let mut conn = Connection::open_in_memory().unwrap();
    upgrade(&mut conn);
    let source = default_source(&conn).unwrap();
    assert_eq!(source.matches("model = ").count(), 1, "{source}");
    assert!(source.lines().any(|l| l == "model = \"opus\""), "{source}");
}

fn at(last: u32) -> Connection {
    let mut conn = Connection::open_in_memory().unwrap();
    let before: Vec<_> = townhall_core::store::migrate::all()
        .into_iter()
        .filter(|m| m.id <= last)
        .collect();
    townhall_core::store::migrate::apply(&mut conn, &before).unwrap();
    conn
}

fn source_of(conn: &Connection, name: &str) -> Option<String> {
    conn.query_row(
        "select source from agents where name = ?1 collate nocase",
        [name],
        |r| r.get(0),
    )
    .ok()
}

fn count_of(conn: &Connection, name: &str) -> i64 {
    conn.query_row(
        "select count(*) from agents where name = ?1 collate nocase",
        [name],
        |r| r.get(0),
    )
    .unwrap()
}

#[test]
fn a_fresh_database_has_builder_default_and_adviser() {
    let mut conn = Connection::open_in_memory().unwrap();
    upgrade(&mut conn);
    let names: Vec<String> = conn
        .prepare("select name from agents order by name")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(names, ["adviser", "builder", "default"]);
    let adviser = source_of(&conn, "adviser").unwrap();
    assert!(
        adviser.lines().any(|l| l == "model = \"fable\""),
        "{adviser}"
    );
    assert!(adviser.contains("permission_mode = \"plan\""), "{adviser}");
}

#[test]
fn a_seeded_adviser_moves_to_the_fable_alias() {
    let mut conn = at(812);
    let seeded = source_of(&conn, "adviser").unwrap();
    assert!(seeded.contains("model = \"claude-fable-5-2\""), "{seeded}");
    upgrade(&mut conn);
    let source = source_of(&conn, "adviser").unwrap();
    assert_eq!(
        source,
        seeded.replace("model = \"claude-fable-5-2\"", "model = \"fable\"")
    );
}

#[test]
fn an_edited_adviser_keeps_its_model_line() {
    for model in ["haiku", "fable", "claude-fable-5-2-custom"] {
        let mut conn = at(812);
        let own = format!("name = \"adviser\"\nadapter = \"claude\"\nmodel = \"{model}\"\n");
        conn.execute(
            "update agents set source = ?1 where name = 'adviser'",
            [&own],
        )
        .unwrap();
        upgrade(&mut conn);
        assert_eq!(source_of(&conn, "adviser").unwrap(), own);
    }
}

#[test]
fn an_existing_install_gets_the_adviser_when_it_migrates() {
    let mut conn = at(809);
    assert_eq!(count_of(&conn, "adviser"), 0);
    upgrade(&mut conn);
    assert_eq!(count_of(&conn, "adviser"), 1);
    assert_eq!(count_of(&conn, "builder"), 1);
}

#[test]
fn an_existing_type_named_adviser_is_left_untouched() {
    let mut conn = at(809);
    let own = "name = \"Adviser\"\nadapter = \"claude\"\nmodel = \"haiku\"\n";
    conn.execute(
        "insert into agents (name, description, source, updated_at) values ('Adviser', '', ?1, 'then')",
        [own],
    )
    .unwrap();
    upgrade(&mut conn);
    assert_eq!(count_of(&conn, "adviser"), 1);
    assert_eq!(source_of(&conn, "adviser").unwrap(), own);
}

#[test]
fn the_seeded_default_type_gains_ask_adviser_once() {
    let mut conn = at(809);
    let seeded = default_source(&conn).unwrap();
    assert!(!seeded.contains("ask_adviser"), "{seeded}");
    upgrade(&mut conn);
    let source = default_source(&conn).unwrap();
    assert_eq!(source.matches("ask_adviser").count(), 1, "{source}");
    assert!(
        source.contains("  \"mcp__townhall__conversation_insights\",\n  \"mcp__townhall__search_history\",\n  \"mcp__townhall__read_conversation\",\n  \"mcp__townhall__ask_adviser\",\n  \"Read\","),
        "{source}"
    );
}

#[test]
fn the_seeded_default_type_gains_the_history_tools_and_prompt_line_once() {
    let mut conn = at(809);
    let seeded = default_source(&conn).unwrap();
    assert!(!seeded.contains("search_history"), "{seeded}");
    upgrade(&mut conn);
    let source = default_source(&conn).unwrap();
    assert_eq!(source.matches("mcp__townhall__search_history").count(), 1);
    assert_eq!(
        source.matches("mcp__townhall__read_conversation").count(),
        1
    );
    assert_eq!(source.matches("call search_history").count(), 1, "{source}");
    let line = source.find("call search_history").unwrap();
    let landing = source.find("- landing finished work is one task").unwrap();
    assert!(line < landing, "{source}");
    upgrade(&mut conn);
    assert_eq!(default_source(&conn).unwrap(), source);
}

#[test]
fn a_default_type_with_its_own_tools_gets_no_history_tools() {
    let own = "name = \"default\"\nadapter = \"claude\"\ntools = [\"Read\"]\nprompt = \"- landing finished work is one task\"\n";
    let mut conn = at(809);
    conn.execute(
        "update agents set source = ?1 where name = 'default'",
        [own],
    )
    .unwrap();
    upgrade(&mut conn);
    assert_eq!(default_source(&conn).unwrap(), own);
}

#[test]
fn a_default_type_that_has_it_or_has_its_own_tools_is_left_alone() {
    let has = "name = \"default\"\nadapter = \"claude\"\ntools = [\n  \"mcp__townhall__conversation_insights\",\n  \"mcp__townhall__search_history\",\n  \"mcp__townhall__ask_adviser\"\n]\n";
    let own = "name = \"default\"\nadapter = \"claude\"\ntools = [\"Read\"]\n";
    for source in [has, own] {
        let mut conn = at(809);
        conn.execute(
            "update agents set source = ?1 where name = 'default'",
            [source],
        )
        .unwrap();
        upgrade(&mut conn);
        assert_eq!(default_source(&conn).unwrap(), source);
    }
}

#[tokio::test]
async fn an_empty_home_gets_a_migrated_database() {
    let dir = TempDir::new().unwrap();
    let store = open(&dir);
    let root = home(&dir).root().to_path_buf();
    assert!(root.join(DB_FILE).is_file());
    assert!(root.join(LOCK_FILE).is_file());
    assert!(
        !root.join("config.json").exists(),
        "no config file is ever written"
    );
    assert_eq!(store.settings().await.unwrap(), seeded());
    assert!(store.repos().await.unwrap().is_empty());
    let (mode, fks, version) = store
        .call(|conn| {
            let mode: String = conn.query_row("pragma journal_mode", [], |r| r.get(0))?;
            let fks: i64 = conn.query_row("pragma foreign_keys", [], |r| r.get(0))?;
            let version: i64 = conn.query_row("pragma user_version", [], |r| r.get(0))?;
            Ok((mode, fks, version))
        })
        .await
        .unwrap();
    let newest = townhall_core::store::migrate::all()
        .last()
        .map_or(0, |m| m.id);
    assert_eq!((mode.as_str(), fks, version), ("wal", 1, i64::from(newest)));
}

#[tokio::test]
async fn a_second_open_fails_while_the_first_is_held() {
    let dir = TempDir::new().unwrap();
    let first = open(&dir);
    let err = Store::open(&home(&dir), EventBus::new()).unwrap_err();
    assert!(matches!(err, Error::Unavailable(_)), "{err:?}");
    assert!(
        err.to_string()
            .starts_with("townhall.db is in use by another townhall"),
        "{err}"
    );
    drop(first);
    let again = open(&dir);
    assert!(again.repos().await.unwrap().is_empty());
}

#[tokio::test]
async fn a_newer_database_refuses_to_open() {
    let dir = TempDir::new().unwrap();
    drop(open(&dir));
    let conn = Connection::open(home(&dir).root().join(DB_FILE)).unwrap();
    conn.pragma_update(None, "user_version", 5000).unwrap();
    drop(conn);
    let err = Store::open(&home(&dir), EventBus::new()).unwrap_err();
    assert_eq!(err.to_string(), "townhall.db is from a newer townhall");
}

#[tokio::test]
async fn config_json_is_imported_once_and_renamed() {
    let dir = TempDir::new().unwrap();
    let repo = init_named(dir.path(), "shop", "main").await;
    write_config(
        &dir,
        json!({"repo": repo, "settings": {"agentsAtOnce": 12, "theme": "x"}}),
    );
    let store = open(&dir);
    let repos = store.repos().await.unwrap();
    assert_eq!(repos.len(), 1);
    assert_eq!(
        (repos[0].name.as_str(), repos[0].base_branch.as_str()),
        ("shop", "main")
    );
    assert_eq!(repos[0].path, repo.canonicalize().unwrap());
    assert_eq!(store.desktop_repo().await.unwrap(), Some(repo.clone()));
    assert_eq!(
        store.settings().await.unwrap(),
        seeded(),
        "only the repo is imported, never settings"
    );
    let h = home(&dir);
    assert!(!h.config_file().exists());
    let kept = fs::read_to_string(h.root().join("config.json.imported")).unwrap();
    assert!(kept.contains("theme"), "the old file is kept as it was");
    drop(store);
    assert_eq!(open(&dir).repos().await.unwrap().len(), 1);
}

#[tokio::test]
async fn a_config_json_with_the_removed_permission_settings_still_imports_and_changes_no_setting() {
    let dir = TempDir::new().unwrap();
    write_config(
        &dir,
        json!({"repo": "/x", "settings": {
            "permissionMode": "bypassPermissions", "workWithoutAsking": true, "agentsAtOnce": 4
        }}),
    );
    let store = open(&dir);
    assert_eq!(store.settings().await.unwrap().agents_at_once, 6);
    assert!(home(&dir).root().join("config.json.imported").exists());
}

#[tokio::test]
async fn a_later_config_json_never_overrides_the_database() {
    let dir = TempDir::new().unwrap();
    let store = open(&dir);
    store
        .set_setting(SettingKey::AgentsAtOnce, json!(3))
        .await
        .unwrap();
    drop(store);
    write_config(
        &dir,
        json!({"repo": "/definitely/not/here", "settings": {"agentsAtOnce": 40}}),
    );
    let store = open(&dir);
    assert_eq!(store.settings().await.unwrap().agents_at_once, 3);
    assert!(
        store.repos().await.unwrap().is_empty(),
        "a gone repo is not registered"
    );
    assert_eq!(
        store.desktop_repo().await.unwrap(),
        Some("/definitely/not/here".into())
    );
    assert!(home(&dir).root().join("config.json.imported").exists());
}

#[tokio::test]
async fn a_corrupt_config_json_is_left_alone() {
    let dir = TempDir::new().unwrap();
    let h = home(&dir);
    h.ensure().unwrap();
    fs::write(h.config_file(), "{nope").unwrap();
    let store = open(&dir);
    assert!(store.repos().await.unwrap().is_empty());
    assert_eq!(fs::read_to_string(h.config_file()).unwrap(), "{nope");
}

#[tokio::test]
async fn settings_round_trip_and_validate() {
    let dir = TempDir::new().unwrap();
    let store = open(&dir);
    let s = store
        .set_setting(SettingKey::AgentsAtOnce, json!(9))
        .await
        .unwrap();
    assert_eq!(s.agents_at_once, 9);
    for (key, bad) in [
        (SettingKey::AgentsAtOnce, json!(0)),
        (SettingKey::SessionPanelOpen, json!("yes")),
    ] {
        let err = store.set_setting(key, bad).await.unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)), "{err:?}");
    }
    drop(store);
    let again = open(&dir);
    assert_eq!(again.settings().await.unwrap().agents_at_once, 9);
    let path = std::path::Path::new("/x/app");
    again.set_desktop_repo(path).await.unwrap();
    assert_eq!(
        again.desktop_repo().await.unwrap(),
        Some(path.to_path_buf())
    );
    assert_eq!(again.settings().await.unwrap().agents_at_once, 9);
}

#[tokio::test]
async fn a_bad_settings_row_reads_as_its_default() {
    let dir = TempDir::new().unwrap();
    let store = open(&dir);
    store
        .call(|conn| {
            conn.execute(
                "insert into settings (key, value, updated_at) values ('agentsAtOnce', 'nope', 'x'),
                 ('notifyWhen', '\"loud\"', 'x'), ('workWithoutAsking', 'true', 'x'), ('permissionMode', '\"plan\"', 'x')",
                [],
            )?;
            Ok(())
        })
        .await
        .unwrap();
    let s = store.settings().await.unwrap();
    assert_eq!(s.agents_at_once, 6);
    assert_eq!(s.notify_when, Settings::default().notify_when);
}
