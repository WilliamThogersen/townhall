use std::path::{Path, PathBuf};
use std::process::Command;

/// Accepts a path only if it is a directory inside a git work tree.
/// Returns the canonical path.
pub fn validate_repo(path: &Path) -> Result<PathBuf, String> {
    let canonical = path
        .canonicalize()
        .map_err(|e| format!("cannot open {}: {e}", path.display()))?;
    if !canonical.is_dir() {
        return Err(format!("{} is not a directory", canonical.display()));
    }
    let out = Command::new("git")
        .arg("-C")
        .arg(&canonical)
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .map_err(|e| format!("cannot run git (is it installed?): {e}"))?;
    if out.status.success() && String::from_utf8_lossy(&out.stdout).trim() == "true" {
        Ok(canonical)
    } else {
        Err(format!("{} is not a git repository", canonical.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_missing_path() {
        let err = validate_repo(Path::new("/definitely/not/here")).unwrap_err();
        assert!(err.contains("cannot open"), "{err}");
    }

    #[test]
    fn rejects_plain_directory() {
        let dir = tempfile::tempdir().unwrap();
        let err = validate_repo(dir.path()).unwrap_err();
        assert!(err.contains("not a git repository"), "{err}");
    }

    #[test]
    fn rejects_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("f");
        std::fs::write(&file, "x").unwrap();
        assert!(validate_repo(&file)
            .unwrap_err()
            .contains("not a directory"));
    }

    #[test]
    fn accepts_git_repo() {
        let dir = tempfile::tempdir().unwrap();
        let ok = Command::new("git")
            .arg("init")
            .arg(dir.path())
            .output()
            .unwrap();
        assert!(ok.status.success());
        let got = validate_repo(dir.path()).unwrap();
        assert_eq!(got, dir.path().canonicalize().unwrap());
    }
}
