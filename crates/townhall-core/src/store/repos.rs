//! The `repos` table. Names are unique ignoring case, paths are canonical and unique.

use std::path::PathBuf;

use rusqlite::{params, Connection, OptionalExtension};

use super::codec::path_text;
use super::Store;
use crate::model::{ForgeKind, PutRepoAccounts, Repo};
use crate::repos::names;
use crate::{clock, Error, Result};

const COLUMNS: &str = "name, path, base_branch, github_account, gitlab_account";

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Repo> {
    Ok(Repo {
        name: r.get(0)?,
        path: PathBuf::from(r.get::<_, String>(1)?),
        base_branch: r.get(2)?,
        github_account: r.get(3)?,
        gitlab_account: r.get(4)?,
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

    /// Picks the registered account this repo uses on a forge; `Some(None)` clears a pick and
    /// `None` leaves it. The login must be in the `forgeAccounts` setting for that forge.
    pub async fn set_repo_accounts(&self, name: &str, put: PutRepoAccounts) -> Result<Repo> {
        let known = self.settings().await?.forge_accounts;
        for (forge, pick) in [
            (ForgeKind::Github, &put.github_account),
            (ForgeKind::Gitlab, &put.gitlab_account),
        ] {
            let Some(Some(login)) = pick else { continue };
            let listed = known
                .iter()
                .any(|a| a.forge == forge && a.login.eq_ignore_ascii_case(login));
            if !listed {
                return Err(Error::InvalidInput(format!(
                    "no {} account `{login}` is registered: add it in settings first",
                    forge.as_str()
                )));
            }
        }
        let name = name.to_string();
        self.call(move |conn| {
            let found = by_name(conn, &name)?.ok_or_else(|| Error::missing("repo", &name))?;
            let github = put.github_account.unwrap_or(found.github_account);
            let gitlab = put.gitlab_account.unwrap_or(found.gitlab_account);
            conn.execute(
                "update repos set github_account = ?1, gitlab_account = ?2 where name = ?3",
                params![github, gitlab, found.name],
            )?;
            by_name(conn, &found.name)?.ok_or_else(|| Error::missing("repo", &found.name))
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::events::EventBus;
    use crate::model::{PutRepoAccounts, Repo, SettingKey};
    use crate::store::Store;
    use crate::{Error, Home};

    async fn store_with_repo() -> (tempfile::TempDir, Store) {
        let dir = tempfile::TempDir::new().unwrap();
        let store = Store::open(&Home::at(dir.path().join(".townhall")), EventBus::new()).unwrap();
        let repo = Repo {
            name: "shop".into(),
            path: "/x/shop".into(),
            base_branch: "main".into(),
            github_account: None,
            gitlab_account: None,
        };
        store.add_repo(repo).await.unwrap();
        let accounts = json!([
            {"forge": "github", "login": "work-me"},
            {"forge": "github", "login": "private-me"},
            {"forge": "gitlab", "login": "work-me", "host": "gitlab.corp.io"}
        ]);
        let set = store.set_setting(SettingKey::ForgeAccounts, accounts);
        set.await.unwrap();
        (dir, store)
    }

    fn pick(github: Option<Option<&str>>, gitlab: Option<Option<&str>>) -> PutRepoAccounts {
        let own = |p: Option<Option<&str>>| p.map(|o| o.map(str::to_string));
        PutRepoAccounts {
            github_account: own(github),
            gitlab_account: own(gitlab),
        }
    }

    #[tokio::test]
    async fn a_repo_picks_a_registered_account_per_forge_and_keeps_the_other() {
        let (_dir, store) = store_with_repo().await;
        let put = pick(Some(Some("private-me")), None);
        let repo = store.set_repo_accounts("SHOP", put).await.unwrap();
        assert_eq!(repo.github_account.as_deref(), Some("private-me"));
        assert_eq!(repo.gitlab_account, None);
        let put = pick(None, Some(Some("work-me")));
        let repo = store.set_repo_accounts("shop", put).await.unwrap();
        assert_eq!(repo.github_account.as_deref(), Some("private-me"));
        assert_eq!(repo.gitlab_account.as_deref(), Some("work-me"));
        assert_eq!(store.repos().await.unwrap(), vec![repo]);
        let put = pick(Some(None), None);
        let cleared = store.set_repo_accounts("shop", put).await.unwrap();
        assert_eq!(cleared.github_account, None);
        assert_eq!(cleared.gitlab_account.as_deref(), Some("work-me"));
    }

    #[tokio::test]
    async fn an_account_nobody_registered_or_on_the_wrong_forge_is_refused() {
        let (_dir, store) = store_with_repo().await;
        let put = pick(Some(Some("stranger")), None);
        let err = store.set_repo_accounts("shop", put).await.unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)), "{err}");
        assert!(err.to_string().contains("add it in settings"), "{err}");
        let put = pick(None, Some(Some("private-me")));
        assert!(store.set_repo_accounts("shop", put).await.is_err());
        let put = pick(Some(Some("work-me")), None);
        let err = store.set_repo_accounts("ghost", put).await.unwrap_err();
        assert!(matches!(err, Error::Missing { .. }), "{err}");
    }
}
