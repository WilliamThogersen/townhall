//! Pushing a task branch and opening its pull request (GitHub) or merge request (GitLab).

use std::io::ErrorKind;
use std::path::Path;

use super::accounts::{Accounts, Picked};
use super::auth_env::{auth_failure, remote_env};
use super::source::split_remote;
use crate::cmd::{probe_blocking, run_blocking, run_blocking_env};
use crate::model::ForgeKind;
use crate::redact::redact;
use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Forge {
    GitHub,
    GitLab,
}

impl Forge {
    pub fn kind(self) -> ForgeKind {
        match self {
            Self::GitHub => ForgeKind::Github,
            Self::GitLab => ForgeKind::Gitlab,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Origin {
    pub forge: Forge,
    pub host: String,
}

pub fn detect(origin_url: &str, configured: Option<&str>) -> Result<Origin> {
    let origin_url = origin_url.trim();
    let host = match split_remote(origin_url) {
        Some((scheme, host, _)) if !scheme.eq_ignore_ascii_case("file") && !host.is_empty() => {
            host.to_ascii_lowercase()
        }
        _ => {
            return Err(Error::InvalidInput(format!(
                "origin `{origin_url}` is a local folder: townhall opens pull requests only on GitHub or GitLab"
            )))
        }
    };
    let forge = match configured.map(str::trim).filter(|c| !c.is_empty()) {
        Some(given) => match given.to_ascii_lowercase().as_str() {
            "github" => Forge::GitHub,
            "gitlab" => Forge::GitLab,
            _ => {
                return Err(Error::InvalidInput(format!(
                    "`townhall.forge` is `{given}`: use `github` or `gitlab`"
                )))
            }
        },
        None if host.contains("github") => Forge::GitHub,
        None if host.contains("gitlab") => Forge::GitLab,
        None => {
            return Err(Error::InvalidInput(format!(
                "cannot tell whether `{host}` is GitHub or GitLab: run `git config townhall.forge gitlab` (or `github`) in the repo"
            )))
        }
    };
    Ok(Origin { forge, host })
}

pub const GH_BIN_ENV: &str = "TOWNHALL_GH_BIN";
pub const GLAB_BIN_ENV: &str = "TOWNHALL_GLAB_BIN";

#[derive(Debug, Clone)]
pub struct Programs {
    pub gh: String,
    pub glab: String,
}

#[derive(Debug, Clone, Copy)]
pub struct PrRequest<'a> {
    pub worktree: &'a std::path::Path,
    pub branch: &'a str,
    pub base: &'a str,
    pub title: &'a str,
    pub body: &'a str,
    /// Which registered account the push and the PR are made as.
    pub accounts: &'a Accounts,
}

/// Environment variables for one child, token included: fetched per spawn, never stored.
type Session = Vec<(String, String)>;

fn with_session(session: &Session) -> Vec<(&str, &str)> {
    let mut envs = remote_env();
    envs.extend(session.iter().map(|(k, v)| (k.as_str(), v.as_str())));
    envs
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Published {
    NoRemote,
    Opened { url: String, number: Option<u64> },
}

impl Programs {
    pub fn from_env() -> Self {
        let pick = |var: &str, default: &str| {
            std::env::var(var)
                .ok()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| default.to_string())
        };
        Self {
            gh: pick(GH_BIN_ENV, "gh"),
            glab: pick(GLAB_BIN_ENV, "glab"),
        }
    }
}

fn invalid(message: String) -> Error {
    Error::InvalidInput(redact(&message).into_owned())
}

fn link_in(text: &str) -> Option<String> {
    text.split_whitespace()
        .rev()
        .find(|t| t.starts_with("http://") || t.starts_with("https://"))
        .map(str::to_string)
}

fn check_login(program: &str, origin: &Origin, dir: &Path) -> Result<()> {
    let host = origin.host.as_str();
    let (name, site) = match origin.forge {
        Forge::GitHub => ("GitHub CLI", "https://cli.github.com"),
        Forge::GitLab => ("GitLab CLI", "https://gitlab.com/gitlab-org/cli"),
    };
    let args = ["auth", "status", "--hostname", host];
    match run_blocking_env(program, &args, Some(dir), &remote_env()) {
        Ok(_) => Ok(()),
        Err(Error::Spawn { source, .. }) if source.kind() == ErrorKind::NotFound => {
            Err(invalid(format!(
                "`{program}` is not installed: install the {name} ({site}), then run `{program} auth login --hostname {host}`"
            )))
        }
        Err(Error::Command { stderr, .. }) => Err(invalid(format!(
            "`{program}` is not logged in to {host}: run `{program} auth login --hostname {host}` ({})",
            stderr.trim()
        ))),
        Err(e) => Err(e),
    }
}

fn active_login(
    program: &str,
    host: &str,
    dir: &Path,
    session: &Session,
) -> Result<(String, String)> {
    let args = [
        "api",
        "--hostname",
        host,
        "user",
        "--jq",
        r#".login + " " + (.id | tostring)"#,
    ];
    let out = run_blocking_env(program, &args, Some(dir), &with_session(session)).map_err(|e| {
        invalid(format!(
            "could not ask `{program}` who is logged in to {host}: {e}"
        ))
    })?;
    let mut parts = out.split_whitespace();
    match (parts.next(), parts.next()) {
        (Some(login), Some(id)) => Ok((login.to_string(), id.to_string())),
        _ => Err(invalid(format!(
            "`{program} api user` printed no login: {}",
            out.trim()
        ))),
    }
}

fn set_git_identity(worktree: &Path, login: &str, id: &str) -> Result<()> {
    let email = format!("{id}+{login}@users.noreply.github.com");
    for (key, value) in [("user.name", login), ("user.email", email.as_str())] {
        run_blocking("git", &["config", key, value], Some(worktree))?;
    }
    Ok(())
}

fn not_logged_in(program: &str, picked: &Picked, why: &str) -> Error {
    let Picked { login, host } = picked;
    invalid(format!(
        "`{program}` has no login for {login} on {host}: run `{program} auth login --hostname {host}` as {login}{why}"
    ))
}

fn token_of(program: &str, args: &[&str], picked: &Picked, dir: &Path) -> Result<String> {
    match run_blocking_env(program, args, Some(dir), &remote_env()) {
        Ok(token) if !token.trim().is_empty() => Ok(token.trim().to_string()),
        Ok(_) => Err(not_logged_in(program, picked, "")),
        Err(Error::Command { stderr, .. }) => Err(not_logged_in(
            program,
            picked,
            &format!(" ({})", redact(stderr.trim())),
        )),
        Err(e) => Err(e),
    }
}

/// `gh` on github.com reads `GH_TOKEN`, on any other host `GH_ENTERPRISE_TOKEN`.
fn github_session(program: &str, picked: &Picked, dir: &Path) -> Result<Session> {
    let args = [
        "auth",
        "token",
        "--user",
        picked.login.as_str(),
        "--hostname",
        picked.host.as_str(),
    ];
    let token = token_of(program, &args, picked, dir)?;
    let var = if picked.host == ForgeKind::Github.default_host() {
        "GH_TOKEN"
    } else {
        "GH_ENTERPRISE_TOKEN"
    };
    Ok(vec![
        (var.to_string(), token),
        ("GH_HOST".to_string(), picked.host.clone()),
    ])
}

fn gitlab_session(program: &str, picked: &Picked, dir: &Path) -> Result<Session> {
    let args = ["config", "get", "token", "--host", picked.host.as_str()];
    let token = token_of(program, &args, picked, dir)?;
    Ok(vec![
        ("GITLAB_TOKEN".to_string(), token),
        ("GITLAB_HOST".to_string(), picked.host.clone()),
    ])
}

fn start_session(program: &str, origin: &Origin, picked: &Picked, dir: &Path) -> Result<Session> {
    let session = match origin.forge {
        Forge::GitHub => github_session(program, picked, dir)?,
        Forge::GitLab => gitlab_session(program, picked, dir)?,
    };
    if origin.forge == Forge::GitHub {
        let (login, id) = active_login(program, &picked.host, dir, &session)?;
        if !login.eq_ignore_ascii_case(&picked.login) {
            return Err(invalid(format!(
                "`{program}` returned a token for {login} on {}, not {}: log in again with `{program} auth login --hostname {}` as {}",
                picked.host, picked.login, picked.host, picked.login
            )));
        }
        set_git_identity(dir, &login, &id)?;
    }
    Ok(session)
}

fn push(req: &PrRequest, session: &Session) -> Result<()> {
    let envs = with_session(session);
    let args = ["push", "--set-upstream", "origin", req.branch];
    match run_blocking_env("git", &args, Some(req.worktree), &envs) {
        Ok(_) => Ok(()),
        Err(Error::Command { stderr, .. }) => {
            let why =
                auth_failure(&stderr).map_or_else(|| stderr.trim().to_string(), str::to_string);
            Err(invalid(format!(
                "could not push {} to origin: {why}",
                req.branch
            )))
        }
        Err(e) => Err(e),
    }
}

fn create(
    req: &PrRequest,
    origin: &Origin,
    programs: &Programs,
    session: &Session,
) -> Result<String> {
    let (program, shown, args, extra): (&str, &str, Vec<&str>, (&str, &str)) = match origin.forge {
        Forge::GitHub => (
            &programs.gh,
            "gh pr create",
            vec![
                "pr", "create", "--head", req.branch, "--base", req.base, "--title", req.title,
                "--body", req.body,
            ],
            ("GH_PROMPT_DISABLED", "1"),
        ),
        Forge::GitLab => (
            &programs.glab,
            "glab mr create",
            vec![
                "mr",
                "create",
                "--source-branch",
                req.branch,
                "--target-branch",
                req.base,
                "--title",
                req.title,
                "--description",
                req.body,
                "--yes",
            ],
            ("GITLAB_HOST", origin.host.as_str()),
        ),
    };
    let mut envs = with_session(session);
    envs.push(extra);
    match run_blocking_env(program, &args, Some(req.worktree), &envs) {
        Ok(out) => link_in(&out)
            .ok_or_else(|| invalid(format!("`{shown}` printed no link: {}", out.trim()))),
        Err(Error::Command { stderr, .. }) => {
            match link_in(&stderr)
                .filter(|_| stderr.to_ascii_lowercase().contains("already exists"))
            {
                Some(url) => Ok(url),
                None => Err(invalid(format!("`{shown}` failed: {}", stderr.trim()))),
            }
        }
        Err(e) => Err(e),
    }
}

/// The forge and host of the worktree's `origin`; `None` when it has none.
pub fn origin_of(worktree: &Path) -> Result<Option<Origin>> {
    let dir = Some(worktree);
    let url = match probe_blocking("git", &["remote", "get-url", "origin"], dir) {
        Ok(url) => url,
        Err(Error::Command { .. }) => return Ok(None),
        Err(e) => return Err(e),
    };
    let configured = probe_blocking("git", &["config", "--get", "townhall.forge"], dir).ok();
    detect(&url, configured.as_deref()).map(Some)
}

pub fn publish(req: &PrRequest, programs: &Programs) -> Result<Published> {
    let Some(origin) = origin_of(req.worktree)? else {
        return Ok(Published::NoRemote);
    };
    let program = match origin.forge {
        Forge::GitHub => &programs.gh,
        Forge::GitLab => &programs.glab,
    };
    let picked = req.accounts.pick(origin.forge.kind(), &origin.host)?;
    check_login(program, &origin, req.worktree)?;
    let session = match &picked {
        Some(picked) => start_session(program, &origin, picked, req.worktree)?,
        None => Session::new(),
    };
    push(req, &session)?;
    let url = create(req, &origin, programs, &session)?;
    Ok(Published::Opened {
        number: number_from_url(&url),
        url,
    })
}

pub fn number_from_url(url: &str) -> Option<u64> {
    let mut segments = url
        .split(['?', '#'])
        .next()?
        .trim_end_matches('/')
        .split('/');
    segments.find(|s| *s == "pull" || *s == "merge_requests")?;
    segments.next()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn forge(url: &str) -> (Forge, String) {
        let o = detect(url, None).unwrap_or_else(|e| panic!("{url}: {e}"));
        (o.forge, o.host)
    }

    #[test]
    fn github_in_every_spelling() {
        for url in [
            "https://github.com/acme/shop",
            "https://github.com/acme/shop.git",
            "git@github.com:acme/shop.git",
            "ssh://git@github.com:22/acme/shop.git",
        ] {
            assert_eq!(forge(url), (Forge::GitHub, "github.com".into()), "{url}");
        }
        assert_eq!(
            forge("https://github.mycorp.com/a/b"),
            (Forge::GitHub, "github.mycorp.com".into())
        );
    }

    #[test]
    fn gitlab_including_self_hosted() {
        assert_eq!(forge("https://gitlab.com/g/sub/app.git").0, Forge::GitLab);
        assert_eq!(forge("git@gitlab.com:g/app.git").0, Forge::GitLab);
        assert_eq!(
            forge("https://gitlab.example.com/g/app"),
            (Forge::GitLab, "gitlab.example.com".into())
        );
        assert_eq!(
            forge("ssh://git@gitlab.corp.io:2222/g/app.git"),
            (Forge::GitLab, "gitlab.corp.io".into())
        );
    }

    #[test]
    fn an_unknown_host_needs_the_config() {
        let err = detect("https://code.example.com/g/app", None).unwrap_err();
        assert!(err.to_string().contains("townhall.forge"), "{err}");
        let got = detect("https://code.example.com/g/app", Some("GitLab")).unwrap();
        assert_eq!(got.forge, Forge::GitLab);
        let got = detect("https://gitlab.example.com/g/app", Some("github")).unwrap();
        assert_eq!(got.forge, Forge::GitHub);
        let err = detect("https://code.example.com/g/app", Some("bitbucket")).unwrap_err();
        assert!(err.to_string().contains("bitbucket"), "{err}");
    }

    #[test]
    fn local_origins_are_refused() {
        for url in ["/srv/x.git", "file:///srv/x.git"] {
            let err = detect(url, None).unwrap_err();
            assert!(
                err.to_string().contains("is a local folder"),
                "{url}: {err}"
            );
        }
    }

    #[test]
    fn numbers_come_from_the_link() {
        assert_eq!(number_from_url("https://github.com/a/b/pull/7"), Some(7));
        assert_eq!(number_from_url("https://github.com/a/b/pull/7/"), Some(7));
        assert_eq!(
            number_from_url("https://gitlab.x/g/a/-/merge_requests/12"),
            Some(12)
        );
        assert_eq!(number_from_url("https://github.com/a/b"), None);
    }
}
