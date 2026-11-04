use std::path::{Path, PathBuf};
use std::process::Command;

/// Accepts a path only if it is the top folder of a git work tree, as the daemon's
/// registry does: it refuses a subfolder, so the picker must too. Returns the canonical path.
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
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|e| format!("cannot run git (is it installed?): {e}"))?;
    let top = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let top = Path::new(&top).canonicalize().ok();
    if out.status.success() && top.as_deref() == Some(canonical.as_path()) {
        Ok(canonical)
    } else {
        Err(format!(
            "{} is not the top folder of a git repo",
            canonical.display()
        ))
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

    fn git_init(dir: &Path) {
        let ok = Command::new("git").arg("init").arg(dir).output().unwrap();
        assert!(ok.status.success());
    }

    #[test]
    fn rejects_plain_directory() {
        let dir = tempfile::tempdir().unwrap();
        let err = validate_repo(dir.path()).unwrap_err();
        assert!(
            err.ends_with("is not the top folder of a git repo"),
            "{err}"
        );
    }

    #[test]
    fn rejects_a_subfolder_of_a_repo() {
        let dir = tempfile::tempdir().unwrap();
        git_init(dir.path());
        let sub = dir.path().join("src");
        std::fs::create_dir(&sub).unwrap();
        let err = validate_repo(&sub).unwrap_err();
        assert!(
            err.ends_with("is not the top folder of a git repo"),
            "{err}"
        );
        assert!(validate_repo(&dir.path().join(".git")).is_err());
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
        git_init(dir.path());
        let got = validate_repo(dir.path()).unwrap();
        assert_eq!(got, dir.path().canonicalize().unwrap());
    }
}
