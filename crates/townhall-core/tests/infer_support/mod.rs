#![allow(dead_code)]

use std::path::{Path, PathBuf};

use tempfile::TempDir;
use townhall_core::repo_infer::{Candidate, Inference, Rule};

use crate::common::{git, init_repo};

/// A temp git repo with `files` committed, plus its candidate entry.
pub struct Repo {
    pub dir: TempDir,
    pub name: &'static str,
}

impl Repo {
    pub fn candidate(&self) -> Candidate {
        candidate(self.name, self.dir.path())
    }
}

pub fn candidate(name: &str, path: &Path) -> Candidate {
    Candidate {
        name: name.to_string(),
        path: path.to_path_buf(),
    }
}

pub async fn repo(name: &'static str, files: &[(&str, &str)]) -> Repo {
    let dir = init_repo().await;
    for (path, body) in files {
        let full = dir.path().join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(&full, body).unwrap();
    }
    if !files.is_empty() {
        git(dir.path(), &["add", "-A"]).await;
        git(
            dir.path(),
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "commit",
                "-m",
                "files",
            ],
        )
        .await;
    }
    Repo { dir, name }
}

pub async fn shop_and_townhall() -> (Repo, Repo) {
    let shop = repo(
        "shop",
        &[
            ("src/search/query.ts", "export const query = 1\n"),
            ("src/routes.ts", "app.post('/api/login', login)\n"),
            (
                "package.json",
                "{\"dependencies\": {\"@shop/ui\": \"1\"}}\n",
            ),
            ("README.md", "shop\n"),
        ],
    )
    .await;
    let townhall = repo(
        "townhall",
        &[
            ("crates/townhall-core/src/lib.rs", "pub mod repo_infer;\n"),
            ("README.md", "townhall\n"),
        ],
    )
    .await;
    (shop, townhall)
}

pub fn found(repo: &str, rule: Rule, reason: &str) -> Inference {
    Inference::Found {
        repo: repo.to_string(),
        rule,
        reason: reason.to_string(),
    }
}

pub fn ask(candidates: &[&str], reason: &str) -> Inference {
    Inference::Ask {
        candidates: candidates.iter().map(|c| c.to_string()).collect(),
        reason: reason.to_string(),
    }
}

pub fn missing_path() -> PathBuf {
    let dir = TempDir::new().unwrap();
    dir.path().join("gone")
}
