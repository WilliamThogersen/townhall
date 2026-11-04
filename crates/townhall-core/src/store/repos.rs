//! The `repos` table. Names are unique ignoring case, paths are canonical and unique.

use std::path::PathBuf;

use rusqlite::{params, Connection, OptionalExtension};

use super::codec::path_text;
use super::Store;
use crate::model::Repo;
use crate::repos::names;
use crate::{clock, Error, Result};

const COLUMNS: &str = "name, path, base_branch";

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Repo> {
    Ok(Repo {
        name: r.get(0)?,
        path: PathBuf::from(r.get::<_, String>(1)?),
        base_branch: r.get(2)?,
    })
}

pub(super) fn list(conn: &Connection) -> Result<Vec<Repo>> {
    let sql = format!("select {COLUMNS} from repos order by added_at, rowid");
    let mut stmt = conn.prepare(&sql)?;
    let repos = stmt.query_map([], row)?;
    Ok(repos.collect::<rusqlite::Result<Vec<Repo>>>()?)
}

fn by_path(conn: &Connection, path: &str) -> Result<Option<Repo>> {
    let sql = format!("select {COLUMNS} from repos where path = ?1");
    Ok(conn.query_row(&sql, [path], row).optional()?)
}

fn by_name(conn: &Connection, name: &str) -> Result<Option<Repo>> {
    let sql = format!("select {COLUMNS} from repos where name = ?1 collate nocase");
    Ok(conn.query_row(&sql, [name], row).optional()?)
}

fn insert(conn: &Connection, repo: &Repo) -> Result<()> {
    if let Some(taken) = by_path(conn, &path_text(&repo.path)?)? {
        return Err(Error::Conflict(format!(
            "{} is already registered as `{}`",
            repo.path.display(),
            taken.name
        )));
    }
    if by_name(conn, &repo.name)?.is_some() {
        return Err(Error::Conflict(format!(
            "a repo named `{}` already exists",
            repo.name
        )));
    }
    conn.execute(
        "insert into repos (name, path, base_branch, added_at) values (?1, ?2, ?3, ?4)",
        params![
            repo.name,
            path_text(&repo.path)?,
            repo.base_branch,
            clock::now_rfc3339()
        ],
    )?;
    Ok(())
}

/// The row for `repo.path`, else `repo` under the first free name.
pub(super) fn ensure(conn: &Connection, repo: Repo) -> Result<Repo> {
    if let Some(found) = by_path(conn, &path_text(&repo.path)?)? {
        return Ok(found);
    }
    let named = Repo {
        name: names::unique(&repo.name, &list(conn)?),
        ..repo
    };
    insert(conn, &named)?;
    Ok(named)
}

impl Store {
    pub async fn repos(&self) -> Result<Vec<Repo>> {
        self.call(|conn| list(conn)).await
    }

    /// `Conflict` when the path, or the name ignoring case, is taken.
    pub async fn add_repo(&self, repo: Repo) -> Result<Repo> {
        self.call(move |conn| {
            let tx = conn.transaction()?;
            insert(&tx, &repo)?;
            tx.commit()?;
            Ok(repo)
        })
        .await
    }

    /// The row for `repo.path`, else `repo` added under the first free name.
    pub async fn ensure_repo(&self, repo: Repo) -> Result<Repo> {
        self.call(move |conn| {
            let tx = conn.transaction()?;
            let found = ensure(&tx, repo)?;
            tx.commit()?;
            Ok(found)
        })
        .await
    }

    /// Removes the repo called `name` (ignoring case). `Missing` when there is none.
    pub async fn remove_repo(&self, name: &str) -> Result<Repo> {
        let name = name.to_string();
        self.call(move |conn| {
            let found = by_name(conn, &name)?.ok_or_else(|| Error::missing("repo", &name))?;
            conn.execute("delete from repos where name = ?1", [&found.name])?;
            Ok(found)
        })
        .await
    }
}
