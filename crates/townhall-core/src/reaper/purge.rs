use std::path::{Path, PathBuf};

use crate::cmd::run;
use crate::{Error, Result};

const ARTIFACTS: [&str; 9] = [
    "target",
    "node_modules",
    "vendor",
    ".next",
    "dist",
    "build",
    ".venv",
    "__pycache__",
    ".gradle",
];

pub(super) async fn purge(worktree: &Path) -> Result<u64> {
    let listed = run(
        "git",
        &[
            "ls-files",
            "-z",
            "--others",
            "--ignored",
            "--exclude-standard",
            "--directory",
        ],
        Some(worktree),
    )
    .await?;
    let folders: Vec<PathBuf> = listed
        .split('\0')
        .filter_map(|entry| entry.strip_suffix('/'))
        .map(|entry| worktree.join(entry))
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| ARTIFACTS.contains(&name))
        })
        .collect();
    tokio::task::spawn_blocking(move || delete_all(&folders))
        .await
        .map_err(|e| Error::io("purge build artifacts", std::io::Error::other(e)))?
}

fn delete_all(folders: &[PathBuf]) -> Result<u64> {
    let mut freed = 0;
    for folder in folders {
        let is_folder = std::fs::symlink_metadata(folder).is_ok_and(|m| m.is_dir());
        if !is_folder {
            continue;
        }
        let size = size_of(folder);
        std::fs::remove_dir_all(folder)
            .map_err(|e| Error::io(format!("delete {}", folder.display()), e))?;
        freed += size;
    }
    Ok(freed)
}

fn size_of(folder: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return 0;
    };
    entries
        .flatten()
        .filter_map(|entry| Some((entry.path(), entry.metadata().ok()?)))
        .map(|(path, meta)| match meta.is_dir() {
            true => size_of(&path),
            false => meta.len(),
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::process::Command;

    use super::purge;

    fn git(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@t"])
            .args(args)
            .current_dir(dir)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    }

    fn write(dir: &Path, file: &str, text: &str) {
        let path = dir.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn repo(ignore: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init", "-q"]);
        write(dir.path(), ".gitignore", ignore);
        write(dir.path(), "src/main.rs", "fn main() {}");
        write(dir.path(), "vendor/lib/lib.php", "<?php");
        git(dir.path(), &["add", "-A"]);
        git(dir.path(), &["commit", "-q", "-m", "init"]);
        dir
    }

    #[tokio::test]
    async fn ignored_artifacts_go_and_everything_else_stays() {
        let dir = repo("target\nnode_modules\ndist\n.venv\n");
        let root = dir.path();
        write(root, "target/debug/app", "12345");
        write(root, "node_modules/dep/index.js", "1234567890");
        write(root, "web/node_modules/dep/index.js", "x");
        write(root, "dist/bundle.js", "x");
        write(root, ".venv/bin/python", "x");
        write(root, "src/notes.txt", "uncommitted source");
        fs::write(root.join("src/main.rs"), "fn main() { edited(); }").unwrap();

        let freed = purge(root).await.unwrap();

        for gone in ["target", "node_modules", "web/node_modules", "dist", ".venv"] {
            assert!(!root.join(gone).exists(), "{gone} should be deleted");
        }
        assert!(root.join("vendor/lib/lib.php").exists(), "tracked vendor");
        assert!(root.join("src/notes.txt").exists(), "untracked source");
        assert_eq!(
            fs::read_to_string(root.join("src/main.rs")).unwrap(),
            "fn main() { edited(); }"
        );
        assert!(root.join(".gitignore").exists());
        assert_eq!(freed, 5 + 10 + 1 + 1 + 1);
    }

    #[tokio::test]
    async fn a_folder_is_deleted_only_when_git_ignores_it() {
        let dir = repo("");
        let root = dir.path();
        write(root, "target/debug/app", "x");
        write(root, "node_modules/dep/index.js", "x");
        write(root, "build/out.o", "x");

        purge(root).await.unwrap();

        for kept in ["target", "node_modules", "build", "vendor"] {
            assert!(root.join(kept).exists(), "{kept} is not ignored");
        }
    }

    #[tokio::test]
    async fn an_ignored_vendor_folder_goes_and_a_tracked_one_stays() {
        let dir = repo("/other/vendor\n");
        let root = dir.path();
        write(root, "other/vendor/pkg.go", "x");
        write(root, "vendor/untracked.txt", "x");

        purge(root).await.unwrap();

        assert!(!root.join("other/vendor").exists());
        assert!(root.join("vendor/lib/lib.php").exists());
        assert!(root.join("vendor/untracked.txt").exists());
    }

    #[tokio::test]
    async fn ignored_names_outside_the_allow_list_stay() {
        let dir = repo("secrets\n*.log\n");
        let root = dir.path();
        write(root, "secrets/key", "x");
        write(root, "run.log", "x");

        purge(root).await.unwrap();

        assert!(root.join("secrets/key").exists());
        assert!(root.join("run.log").exists());
    }

    #[tokio::test]
    async fn a_folder_that_is_not_a_repo_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        assert!(purge(dir.path()).await.is_err());
    }
}
