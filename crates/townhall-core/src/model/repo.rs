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
}

/// `POST /api/repos` body.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AddRepo {
    pub path: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub base_branch: Option<String>,
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
        };
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(
            v,
            json!({"name": "shop", "path": "/x/shop", "baseBranch": "develop"})
        );
        assert_eq!(serde_json::from_value::<Repo>(v).unwrap(), r);
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
