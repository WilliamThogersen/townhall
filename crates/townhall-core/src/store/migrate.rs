//! Embedded SQL migrations. Each unit owns a number range and lists its migrations in its
//! own `unit_*.rs` file, so parallel units never pick the same number or edit a shared list.
//! Applied ones are recorded in `schema_migrations`, so a range that lands later is still
//! applied; `user_version` holds the highest applied number. A database with a migration
//! this build does not know is from a newer townhall and is refused.

use std::ops::RangeInclusive;

use rusqlite::{params, Connection};

use super::{
    unit_2_2, unit_2_3, unit_2_5, unit_2_6, unit_2_7, unit_2_8b, unit_2_8c, unit_2_9, unit_graph,
    unit_history, unit_permissions, unit_plugins,
};
use crate::{clock, Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Migration {
    pub id: u32,
    /// The file name without `.sql`, starting with the zero-padded id: `0002_conversations`.
    pub name: &'static str,
    pub sql: &'static str,
}

const BASE: &[Migration] = &[
    Migration {
        id: 1,
        name: "0001_init",
        sql: include_str!("migrations/0001_init.sql"),
    },
    Migration {
        id: 2,
        name: "0002_conversations",
        sql: include_str!("migrations/0002_conversations.sql"),
    },
    Migration {
        id: 3,
        name: "0003_github_account",
        sql: include_str!("migrations/0003_github_account.sql"),
    },
    Migration {
        id: 4,
        name: "0004_repo_accounts",
        sql: include_str!("migrations/0004_repo_accounts.sql"),
    },
];

/// Who owns which numbers. Ask the orchestrator for a range before adding a unit.
pub const RANGES: [(&str, RangeInclusive<u32>, &[Migration]); 13] = [
    ("2.0 storage, 2.1 contract", 1..=99, BASE),
    ("2.2 conversations", 100..=199, unit_2_2::MIGRATIONS),
    ("2.3 tasks", 200..=299, unit_2_3::MIGRATIONS),
    ("2.5 agent states", 500..=599, unit_2_5::MIGRATIONS),
    ("2.6 attachments", 600..=699, unit_2_6::MIGRATIONS),
    ("2.7 steps and stats", 700..=799, unit_2_7::MIGRATIONS),
    ("2.8b library", 800..=849, unit_2_8b::MIGRATIONS),
    ("2.8c runs", 850..=899, unit_2_8c::MIGRATIONS),
    ("2.9 artifacts", 900..=949, unit_2_9::MIGRATIONS),
    ("plugins", 950..=999, unit_plugins::MIGRATIONS),
    ("history search", 1000..=1049, unit_history::MIGRATIONS),
    ("history graph", 1050..=1099, unit_graph::MIGRATIONS),
    ("permissions", 1100..=1149, unit_permissions::MIGRATIONS),
];

pub const NEWER: &str = "townhall.db is from a newer townhall";

const LEDGER: &str = "create table if not exists schema_migrations (
  id         integer primary key,
  name       text not null,
  applied_at text not null
)";

/// Every known migration, in number order.
pub fn all() -> Vec<Migration> {
    let mut list: Vec<Migration> = RANGES
        .iter()
        .flat_map(|(_, _, m)| m.iter().copied())
        .collect();
    list.sort_by_key(|m| m.id);
    list
}

fn applied(conn: &Connection) -> Result<Vec<u32>> {
    let mut stmt = conn.prepare("select id from schema_migrations order by id")?;
    let ids = stmt.query_map([], |r| r.get::<_, u32>(0))?;
    Ok(ids.collect::<rusqlite::Result<Vec<u32>>>()?)
}

fn check_names(conn: &Connection, known: &[Migration]) -> Result<()> {
    let mut stmt = conn.prepare("select id, name from schema_migrations order by id")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, u32>(0)?, r.get::<_, String>(1)?)))?;
    for row in rows {
        let (id, recorded) = row?;
        if let Some(m) = known.iter().find(|m| m.id == id && m.name != recorded) {
            return Err(Error::Database(format!(
                "townhall.db recorded migration {id} as `{recorded}`, but this build's {id} is `{}`: \
                 the database came from a build with a different migration {id} and cannot be \
                 upgraded; restore a backup or start from a new database",
                m.name
            )));
        }
    }
    Ok(())
}

fn user_version(conn: &Connection) -> Result<u32> {
    Ok(conn.pragma_query_value(None, "user_version", |r| r.get(0))?)
}

/// Applies every missing migration, each in its own transaction. Returns how many ran.
pub fn apply(conn: &mut Connection, known: &[Migration]) -> Result<usize> {
    conn.execute_batch(LEDGER)?;
    let done = applied(conn)?;
    let max_known = known.iter().map(|m| m.id).max().unwrap_or(0);
    let unknown = done.iter().any(|id| !known.iter().any(|m| m.id == *id));
    let mut top = user_version(conn)?;
    if unknown || top > max_known {
        return Err(Error::Unavailable(NEWER.into()));
    }
    check_names(conn, known)?;
    let pending: Vec<&Migration> = known.iter().filter(|m| !done.contains(&m.id)).collect();
    for m in &pending {
        top = top.max(m.id);
        let tx = conn.transaction()?;
        tx.execute_batch(m.sql)
            .map_err(|e| Error::Database(format!("migration {} failed: {e}", m.name)))?;
        tx.execute(
            "insert into schema_migrations (id, name, applied_at) values (?1, ?2, ?3)",
            params![m.id, m.name, clock::now_rfc3339()],
        )?;
        tx.pragma_update(None, "user_version", top)?;
        tx.commit()?;
    }
    Ok(pending.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_migration_is_in_its_units_range_and_named_by_its_number() {
        let mut seen = vec![];
        for (unit, range, list) in RANGES {
            for m in list {
                assert!(
                    range.contains(&m.id),
                    "{unit}: {} outside {range:?}",
                    m.name
                );
                assert!(m.name.starts_with(&format!("{:04}_", m.id)), "{}", m.name);
                assert!(!seen.contains(&m.id), "{} used twice", m.id);
                seen.push(m.id);
            }
        }
        let ranges: Vec<_> = RANGES.iter().map(|(_, r, _)| r.clone()).collect();
        for (i, a) in ranges.iter().enumerate() {
            for b in &ranges[i + 1..] {
                assert!(a.end() < b.start(), "{a:?} overlaps {b:?}");
            }
        }
    }

    #[test]
    fn applies_once_and_late_ranges_still_apply() {
        let mut conn = Connection::open_in_memory().unwrap();
        let early = [BASE[0]];
        assert_eq!(apply(&mut conn, &early).unwrap(), 1);
        let late = Migration {
            id: 150,
            name: "0150_x",
            sql: "create table x (a integer)",
        };
        let mut both = vec![BASE[0], late];
        assert_eq!(apply(&mut conn, &both).unwrap(), 1);
        both.insert(1, BASE[1]);
        assert_eq!(apply(&mut conn, &both).unwrap(), 1, "0002 lands after 0150");
        assert_eq!(apply(&mut conn, &both).unwrap(), 0);
        assert_eq!(user_version(&conn).unwrap(), 150);
    }

    #[test]
    fn a_newer_database_is_refused() {
        let mut conn = Connection::open_in_memory().unwrap();
        apply(&mut conn, &all()).unwrap();
        conn.pragma_update(None, "user_version", 9999).unwrap();
        let err = apply(&mut conn, &all()).unwrap_err();
        assert_eq!(err.to_string(), NEWER);
        let mut other = Connection::open_in_memory().unwrap();
        let extra = Migration {
            id: 98,
            name: "0098_y",
            sql: "create table y (a integer)",
        };
        apply(&mut other, &[BASE[0], extra]).unwrap();
        assert!(matches!(
            apply(&mut other, &[BASE[0]]),
            Err(Error::Unavailable(_))
        ));
    }

    #[test]
    fn a_database_that_ran_the_old_0203_task_blocked_is_refused_with_both_names() {
        let mut conn = Connection::open_in_memory().unwrap();
        let before: Vec<Migration> = all().into_iter().filter(|m| m.id < 203).collect();
        apply(&mut conn, &before).unwrap();
        conn.execute_batch(
            "alter table tasks add column blocked text;
             insert into schema_migrations (id, name, applied_at)
             values (203, '0203_task_blocked', 'then');
             pragma user_version = 203;",
        )
        .unwrap();
        let err = apply(&mut conn, &all()).unwrap_err().to_string();
        assert!(
            err.contains("0203_task_blocked") && err.contains("0203_task_ended_at"),
            "{err}"
        );
        assert_eq!(applied(&conn).unwrap().last(), Some(&203));
    }

    #[test]
    fn migration_0205_applies_after_the_higher_ranges_and_reruns_safely() {
        let mut conn = Connection::open_in_memory().unwrap();
        let without: Vec<Migration> = all().into_iter().filter(|m| m.id != 205).collect();
        apply(&mut conn, &without).unwrap();
        assert!(user_version(&conn).unwrap() >= 900);
        assert_eq!(apply(&mut conn, &all()).unwrap(), 1);
        assert!(conn.prepare("select pr_base from tasks").is_ok());
        assert_eq!(apply(&mut conn, &all()).unwrap(), 0);
        assert_eq!(applied(&conn).unwrap().len(), all().len());
    }

    #[test]
    fn migration_0004_keeps_existing_repos_and_leaves_their_accounts_unpicked() {
        let mut conn = Connection::open_in_memory().unwrap();
        let before: Vec<Migration> = all().into_iter().filter(|m| m.id < 4).collect();
        apply(&mut conn, &before).unwrap();
        conn.execute(
            "insert into repos (name, path, base_branch, added_at) values ('shop', '/x/shop', 'main', 't')",
            [],
        )
        .unwrap();
        assert_eq!(apply(&mut conn, &all()).unwrap(), all().len() - 3);
        let picks: (Option<String>, Option<String>) = conn
            .query_row(
                "select github_account, gitlab_account from repos where name = 'shop'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(picks, (None, None));
        let old: String = conn
            .query_row(
                "select value from settings where key = 'githubAccount'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(old, "\"WilliamThogersen\"");
    }

    #[test]
    fn migration_0206_gives_old_tasks_the_default_retries_and_no_check() {
        let mut conn = Connection::open_in_memory().unwrap();
        let without: Vec<Migration> = all().into_iter().filter(|m| m.id != 206).collect();
        apply(&mut conn, &without).unwrap();
        conn.pragma_update(None, "foreign_keys", false).unwrap();
        conn.execute(
            "insert into tasks (name, conversation_id, title, prompt, repo, base_branch, after,
             state, attachments, created_at, agent)
             values ('a', 'c', 't', 'p', 'r', 'main', '[]', 'queued', '[]', 'x', 'builder')",
            [],
        )
        .unwrap();
        assert_eq!(apply(&mut conn, &all()).unwrap(), 1);
        let row: (Option<String>, Option<String>, i64, Option<String>) = conn
            .query_row(
                "select done_when, check_cmd, max_retries, verify from tasks",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!(row, (None, None, 2, None));
    }

    #[test]
    fn a_database_from_before_0950_opens_and_gains_the_plugin_tables() {
        let mut conn = Connection::open_in_memory().unwrap();
        let before: Vec<Migration> = all().into_iter().filter(|m| m.id < 950).collect();
        apply(&mut conn, &before).unwrap();
        assert!(user_version(&conn).unwrap() < 950);
        assert!(conn.prepare("select name from marketplaces").is_err());
        assert_eq!(apply(&mut conn, &all()).unwrap(), 8);
        assert!(conn
            .prepare("select name, source from marketplaces")
            .is_ok());
        assert!(conn.prepare("select id, marketplace from plugins").is_ok());
        assert!(conn
            .prepare("select plugin_id, key, value, updated_at from plugin_settings")
            .is_ok());
        assert!(conn
            .prepare("select plugin_id, key, value, updated_at from plugin_storage")
            .is_ok());
        assert!(conn
            .prepare(
                "select token_hash, plugin_id, task, created_at, revoked_at from plugin_tokens"
            )
            .is_ok());
        assert_eq!(user_version(&conn).unwrap(), 1052);
        assert_eq!(apply(&mut conn, &all()).unwrap(), 0);
    }

    #[test]
    fn a_database_with_0950_is_refused_by_a_build_without_it() {
        let mut conn = Connection::open_in_memory().unwrap();
        apply(&mut conn, &all()).unwrap();
        let older: Vec<Migration> = all().into_iter().filter(|m| m.id < 950).collect();
        let err = apply(&mut conn, &older).unwrap_err();
        assert!(matches!(err, Error::Unavailable(_)), "{err}");
        assert_eq!(err.to_string(), NEWER);
    }

    #[test]
    fn a_failing_migration_changes_nothing() {
        let mut conn = Connection::open_in_memory().unwrap();
        let bad = Migration {
            id: 3,
            name: "0003_bad",
            sql: "create table z (a); nonsense",
        };
        assert!(apply(&mut conn, &[BASE[0], bad]).is_err());
        assert_eq!(applied(&conn).unwrap(), vec![1]);
        assert_eq!(user_version(&conn).unwrap(), 1);
    }
}
