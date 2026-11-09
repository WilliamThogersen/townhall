//! Repo names: `[A-Za-z0-9._-]`, starting with a letter or digit, unique ignoring case.

use std::path::Path;

use crate::model::Repo;
use crate::{Error, Result};

const MAX_LEN: usize = 64;
const FALLBACK: &str = "repo";

fn allowed(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')
}

pub fn validate(name: &str) -> Result<()> {
    let ok = name.len() <= MAX_LEN
        && name.starts_with(|c: char| c.is_ascii_alphanumeric())
        && name.chars().all(allowed);
    if ok {
        Ok(())
    } else {
        Err(Error::InvalidInput(format!(
            "invalid repo name `{name}`: use letters, digits, `.`, `-` and `_`, starting with a letter or digit"
        )))
    }
}

/// The folder name made safe: other characters become `-`.
pub fn from_folder(path: &Path) -> String {
    let raw = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let safe: String = raw
        .chars()
        .map(|c| if allowed(c) { c } else { '-' })
        .collect();
    let trimmed = safe.trim_start_matches(|c: char| !c.is_ascii_alphanumeric());
    let cut: String = trimmed.chars().take(MAX_LEN).collect();
    if cut.is_empty() {
        FALLBACK.to_string()
    } else {
        cut
    }
}

pub fn same(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

pub fn taken(name: &str, repos: &[Repo]) -> bool {
    repos.iter().any(|r| same(&r.name, name))
}

/// `base`, else `base-2`, `base-3` ... whichever is free.
pub fn unique(base: &str, repos: &[Repo]) -> String {
    (1..)
        .map(|n| match n {
            1 => base.to_string(),
            _ => format!("{base}-{n}"),
        })
        .find(|candidate| !taken(candidate, repos))
        .unwrap_or_else(|| base.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo(name: &str) -> Repo {
        Repo {
            name: name.into(),
            path: format!("/{name}").into(),
            base_branch: "main".into(),
            github_account: None,
            gitlab_account: None,
        }
    }

    #[test]
    fn folder_names_become_safe() {
        assert_eq!(from_folder(Path::new("/x/my app")), "my-app");
        assert_eq!(from_folder(Path::new("/x/.tmpAb1")), "tmpAb1");
        assert_eq!(from_folder(Path::new("/")), "repo");
        assert_eq!(
            from_folder(Path::new(&format!("/x/{}", "a".repeat(80)))).len(),
            64
        );
        assert!(validate(&from_folder(Path::new("/x/émoji ok"))).is_ok());
    }

    #[test]
    fn validation() {
        for good in ["shop", "a.b-c_d", "9lives"] {
            assert!(validate(good).is_ok(), "{good}");
        }
        for bad in ["", "-x", ".x", "a b", "a/b", &"a".repeat(65)] {
            assert!(validate(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn unique_skips_taken_names_ignoring_case() {
        let repos = [repo("App"), repo("app-2")];
        assert_eq!(unique("app", &repos), "app-3");
        assert_eq!(unique("web", &repos), "web");
    }
}
