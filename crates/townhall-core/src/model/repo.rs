use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// A registered repository: one row of the `repos` table, `GET /api/repos`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Repo {
    /// Unique (ignoring case), `[A-Za-z0-9._-]`, the folder name by default.
    pub name: String,
    /// Canonical absolute path.
    pub path: PathBuf,
    /// Where task worktrees start: origin's HEAD branch by default, else the current branch.
    pub base_branch: String,
    /// The login of the registered GitHub account this repo uses; `null` means the first
    /// registered account for the remote's host, else the old `githubAccount` setting.
    #[serde(default)]
    pub github_account: Option<String>,
    /// Same for GitLab; there is no older setting to fall back to.
    #[serde(default)]
    pub gitlab_account: Option<String>,
}

/// `PUT /api/repos/{name}/accounts` body: a missing key leaves that pick as it is, `null`
/// clears it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PutRepoAccounts {
    #[serde(default, deserialize_with = "super::present")]
    pub github_account: Option<Option<String>>,
    #[serde(default, deserialize_with = "super::present")]
    pub gitlab_account: Option<Option<String>>,
}

/// `PATCH /api/repos/{name}` body: a missing key leaves that field as it is.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PatchRepo {
    /// The new name; tasks and conversations that mention the repo follow it.
    #[serde(default)]
    pub name: Option<String>,
    /// The new base branch: it must exist in the repo.
    #[serde(default)]
    pub base_branch: Option<String>,
    /// Without `moveFolder`: where the repo is now (same git repo, new place). With it: where
    /// townhall moves the folder to (must not exist yet).
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub move_folder: bool,
}

/// `POST /api/repos` body.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AddRepo {
    /// A folder (`~` allowed) or a git link; the daemon tells them apart.
    pub path: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub base_branch: Option<String>,
    /// Only for a git link: the folder to clone into (`~` allowed), else the `clonesDir`
    /// setting, else `<home>/repos`.
    #[serde(default)]
    pub clone_dir: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn repo_is_camel_case() {
        let r = Repo {
            name: "shop".into(),
            path: "/x/shop".into(),
            base_branch: "develop".into(),
            github_account: Some("work-me".into()),
            gitlab_account: None,
        };
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(
            v,
            json!({
                "name": "shop", "path": "/x/shop", "baseBranch": "develop",
                "githubAccount": "work-me", "gitlabAccount": null
            })
        );
        assert_eq!(serde_json::from_value::<Repo>(v).unwrap(), r);
        let old = json!({"name": "shop", "path": "/x/shop", "baseBranch": "develop"});
        let read: Repo = serde_json::from_value(old).unwrap();
        assert_eq!((read.github_account, read.gitlab_account), (None, None));
    }

    #[test]
    fn put_accounts_tells_a_missing_key_from_a_null() {
        let p: PutRepoAccounts =
            serde_json::from_value(json!({"githubAccount": "a", "gitlabAccount": null})).unwrap();
        assert_eq!(p.github_account, Some(Some("a".to_string())));
        assert_eq!(p.gitlab_account, Some(None));
        let none: PutRepoAccounts = serde_json::from_value(json!({})).unwrap();
        assert_eq!(none, PutRepoAccounts::default());
        assert!(serde_json::from_value::<PutRepoAccounts>(json!({"token": "x"})).is_err());
    }

    #[test]
    fn add_repo_rejects_unknown_fields() {
        let ok: AddRepo = serde_json::from_value(json!({"path": "/x"})).unwrap();
        assert_eq!(ok.name, None);
        assert!(
            serde_json::from_value::<AddRepo>(json!({"path": "/x", "base_branch": "m"})).is_err()
        );
    }
}
