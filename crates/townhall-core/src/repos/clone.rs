//! Cloning a remote into `<home>/repos/<name>`. Blocking (git): call from `spawn_blocking`.

use std::path::{Path, PathBuf};

use super::source::{classify, Remote, Source};
use crate::cmd::{run_blocking, run_blocking_env};
use crate::redact::redact;
use crate::{Error, Home, Result};

/// Never wait for a username or password on a terminal nobody is looking at, and make ssh
/// fail instead of asking for a passphrase or to trust a host. A `GIT_SSH_COMMAND` the user
/// already has is kept: it is theirs.
const CLONE_ENV: [(&str, &str); 1] = [("GIT_TERMINAL_PROMPT", "0")];
const SSH_NO_PROMPT: &str = "ssh -o BatchMode=yes";

const AUTH_FAILED: [&str; 5] = [
    "authentication failed",
    "could not read username",
    "could not read password",
    "terminal prompts disabled",
    "permission denied",
];
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

fn canonical(path: &Path) -> Result<PathBuf> {
    path.canonicalize()
        .map_err(|e| Error::io(format!("clone {}", path.display()), e))
}

fn is_clone_of(dir: &Path, remote: &Remote) -> bool {
    let origin = run_blocking("git", &["config", "--get", "remote.origin.url"], Some(dir));
    match origin.map(|url| classify(&url)) {
        Ok(Ok(Source::Remote(found))) => found.identity == remote.identity,
        _ => false,
    }
}

fn clone_into(remote: &Remote, dest: &Path) -> Result<()> {
    let target = dest.to_string_lossy();
    let ssh = std::env::var_os("GIT_SSH_COMMAND").is_none();
    let mut envs = CLONE_ENV.to_vec();
    if ssh {
        envs.push(("GIT_SSH_COMMAND", SSH_NO_PROMPT));
    }
    let args = ["clone", "-q", "--", &remote.url, &target];
    match run_blocking_env("git", &args, None, &envs) {
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
    let why = if has(&AUTH_FAILED) {
        "authentication failed: check your git credentials or ssh key".to_string()
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
}
