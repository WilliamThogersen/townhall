//! Cloning a remote into `<home>/repos/<name>`. Blocking (git): call from `spawn_blocking`.

use std::path::{Path, PathBuf};

use super::auth_env::{auth_failure, remote_env};
use super::source::{classify, Remote, Source};
use crate::cmd::{probe_blocking, run_blocking_env};
use crate::redact::redact;
use crate::{Error, Home, Result};

/// Never wait for a username or password on a terminal nobody is looking at, and make ssh
/// fail instead of asking for a passphrase or to trust a host. A `GIT_SSH_COMMAND` the user
/// already has is kept: it is theirs.
pub(super) const CLONE_ENV: [(&str, &str); 1] = [("GIT_TERMINAL_PROMPT", "0")];
pub(super) const SSH_NO_PROMPT: &str = "ssh -o BatchMode=yes";

const NOT_FOUND: [&str; 3] = [
    "not found",
    "does not exist",
    "does not appear to be a git repository",
];

/// The folder holding a clone of `remote`: an existing clone of the same repo is reused,
/// else `remote` is cloned into the first free `<name>`, `<name>-2`, ...
pub fn clone_or_reuse(home: &Home, remote: &Remote) -> Result<PathBuf> {
    let root = home.repos_dir()?;
    for n in 1.. {
        let folder = match n {
            1 => remote.name.clone(),
            _ => format!("{}-{n}", remote.name),
        };
        let dest = root.join(folder);
        if !dest.exists() {
            clone_into(remote, &dest)?;
            return canonical(&dest);
        }
        if is_clone_of(&dest, remote) {
            return canonical(&dest);
        }
    }
    unreachable!("an unbounded range ends only by returning")
}

/// Where `remote` is cloned inside `base`: `<base>/<name>`.
pub fn clone_target(base: &Path, remote: &Remote) -> PathBuf {
    base.join(&remote.name)
}

/// Clones `remote` into `<base>/<name>`, creating `base` when missing. An existing clone of the
/// same repo there is reused; any other existing target is refused (`Conflict`), never
/// overwritten or renamed.
pub fn clone_into_base(base: &Path, remote: &Remote) -> Result<PathBuf> {
    let dest = clone_target(base, remote);
    if dest.exists() {
        if is_clone_of(&dest, remote) {
            return canonical(&dest);
        }
        return Err(Error::Conflict(format!(
            "{} already exists and is not a clone of {}: pick another folder",
            dest.display(),
            remote.url
        )));
    }
    std::fs::create_dir_all(base)
        .map_err(|e| Error::io(format!("create {}", base.display()), e))?;
    clone_into(remote, &dest)?;
    canonical(&dest)
}

fn canonical(path: &Path) -> Result<PathBuf> {
    path.canonicalize()
        .map_err(|e| Error::io(format!("clone {}", path.display()), e))
}

fn is_clone_of(dir: &Path, remote: &Remote) -> bool {
    let origin = probe_blocking("git", &["config", "--get", "remote.origin.url"], Some(dir));
    match origin.map(|url| classify(&url)) {
        Ok(Ok(Source::Remote(found))) => found.identity == remote.identity,
        _ => false,
    }
}

fn clone_into(remote: &Remote, dest: &Path) -> Result<()> {
    clone_into_with("git", remote, dest, &[])
}

pub(super) fn clone_into_with(
    git: &str,
    remote: &Remote,
    dest: &Path,
    extra: &[(&str, &str)],
) -> Result<()> {
    let target = dest.to_string_lossy();
    let mut envs: Vec<(&str, &str)> = remote_env();
    envs.extend_from_slice(extra);
    let args = ["clone", "-q", "--", &remote.url, &target];
    match run_blocking_env(git, &args, None, &envs) {
        Ok(_) => Ok(()),
        Err(Error::Command { stderr, .. }) => {
            let _ = std::fs::remove_dir_all(dest);
            Err(clone_failed(&remote.url, &stderr))
        }
        Err(e) => Err(e),
    }
}

fn clone_failed(url: &str, stderr: &str) -> Error {
    let lower = stderr.to_ascii_lowercase();
    let has = |needles: &[&str]| needles.iter().any(|n| lower.contains(n));
    let why = if let Some(hint) = auth_failure(stderr, url.to_ascii_lowercase().starts_with("http"))
    {
        hint.to_string()
    } else if has(&NOT_FOUND) {
        "repository not found: check the link and your access to it".to_string()
    } else {
        stderr.trim().to_string()
    };
    Error::InvalidInput(redact(&format!("could not clone {url}: {why}")).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git(dir: &Path, args: &[&str]) {
        let status = std::process::Command::new("git")
            .args([
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .current_dir(dir)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    }

    /// A local "remote": a repo with one commit, reachable as a `file://` url.
    fn origin(root: &Path, name: &str) -> Remote {
        let dir = root.join(format!("{name}.git"));
        std::fs::create_dir_all(&dir).unwrap();
        git(&dir, &["init", "-q", "-b", "main"]);
        git(&dir, &["commit", "-q", "--allow-empty", "-m", "first"]);
        match classify(&format!("file://{}", dir.display())).unwrap() {
            Source::Remote(r) => r,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_clone_target_is_the_repo_name_inside_the_base_folder() {
        let remote = classify("git@github.com:acme/shop.git").unwrap();
        let Source::Remote(remote) = remote else {
            panic!()
        };
        assert_eq!(
            clone_target(Path::new("/dev/projects"), &remote),
            PathBuf::from("/dev/projects/shop")
        );
    }

    #[test]
    fn a_clone_lands_in_the_base_folder_which_is_created_when_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let remote = origin(tmp.path(), "shop");
        let base = tmp.path().join("nested/dev");
        let got = clone_into_base(&base, &remote).unwrap();
        assert_eq!(got, base.join("shop").canonicalize().unwrap());
        assert!(got.join(".git").exists());
    }

    #[test]
    fn an_existing_target_folder_is_never_overwritten() {
        let tmp = tempfile::tempdir().unwrap();
        let remote = origin(tmp.path(), "shop");
        let base = tmp.path().join("dev");
        std::fs::create_dir_all(base.join("shop")).unwrap();
        std::fs::write(base.join("shop/keep.txt"), "mine").unwrap();
        let err = clone_into_base(&base, &remote).unwrap_err();
        assert!(matches!(err, Error::Conflict(_)), "{err:?}");
        assert!(err.to_string().contains("already exists"), "{err}");
        assert_eq!(
            std::fs::read_to_string(base.join("shop/keep.txt")).unwrap(),
            "mine"
        );
    }

    #[test]
    fn an_existing_clone_of_the_same_repo_is_reused() {
        let tmp = tempfile::tempdir().unwrap();
        let remote = origin(tmp.path(), "shop");
        let base = tmp.path().join("dev");
        let first = clone_into_base(&base, &remote).unwrap();
        assert_eq!(clone_into_base(&base, &remote).unwrap(), first);
    }

    #[test]
    fn prompts_are_always_off() {
        assert!(CLONE_ENV.contains(&("GIT_TERMINAL_PROMPT", "0")));
    }

    #[test]
    fn failures_are_told_apart() {
        let msg = |stderr| clone_failed("u", stderr).to_string();
        let auth = msg(
            "fatal: could not read Username for 'https://github.com': terminal prompts disabled",
        );
        assert!(auth.contains("authentication failed"), "{auth}");
        let auth = msg("git@github.com: Permission denied (publickey).");
        assert!(auth.contains("authentication failed"), "{auth}");
        let gone = msg("remote: Repository not found.\nfatal: repository 'x' not found");
        assert!(gone.contains("repository not found"), "{gone}");
        let other = msg("fatal: unable to access: Could not resolve host: nowhere");
        assert!(other.contains("Could not resolve host"), "{other}");
    }

    #[test]
    fn an_https_clone_that_cannot_log_in_suggests_ssh() {
        let stderr = "fatal: could not read Username for 'https://gitlab.example.com': terminal prompts disabled";
        let https = clone_failed("https://gitlab.example.com/konpap/vines.git", stderr).to_string();
        assert!(https.contains("switch to ssh"), "{https}");
        let ssh = clone_failed("git@gitlab.example.com:konpap/vines.git", stderr).to_string();
        assert!(!ssh.contains("switch to ssh"), "{ssh}");
    }
}
