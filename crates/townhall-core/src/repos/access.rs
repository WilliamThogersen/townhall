use std::time::Duration;

use super::accounts::Picked;
use super::detect::{capture_env, detect_blocking, Detected};
use super::publish::{github_session, gitlab_session, Programs};
use super::source::split_remote;
use crate::model::{ForgeAccount, ForgeKind};

const PROBE_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Located {
    pub forge: ForgeKind,
    pub host: String,
    pub path: String,
}

fn parts(url: &str) -> Option<(String, String)> {
    let (scheme, host, path) = split_remote(url.trim())?;
    if scheme.eq_ignore_ascii_case("file") || host.is_empty() {
        return None;
    }
    let path = path.trim_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    if path.split('/').count() < 2 || path.split('/').any(str::is_empty) {
        return None;
    }
    Some((host.to_ascii_lowercase(), path.to_string()))
}

pub fn discover(
    programs: &Programs,
    url: &str,
    registered: &[ForgeAccount],
) -> Option<(Located, Outcome)> {
    parts(url)?;
    let detection = detect_blocking(programs, registered);
    let gitlab_hosts: Vec<String> = detection
        .accounts
        .iter()
        .filter(|d| d.forge == ForgeKind::Gitlab)
        .map(|d| d.host.clone())
        .collect();
    let located = locate(url, &gitlab_hosts)?;
    let outcome = find_account(programs, &located, registered, &detection.accounts);
    Some((located, outcome))
}

pub fn locate(url: &str, gitlab_hosts: &[String]) -> Option<Located> {
    let (host, path) = parts(url)?;
    let listed = gitlab_hosts.iter().any(|h| h.eq_ignore_ascii_case(&host));
    let forge = if host == ForgeKind::Github.default_host() {
        ForgeKind::Github
    } else if listed || host.contains("gitlab") {
        ForgeKind::Gitlab
    } else {
        return None;
    };
    Some(Located { forge, host, path })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub account: Picked,
    pub registered: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Outcome {
    pub found: Option<Found>,
    pub tried: Vec<Picked>,
}

impl Outcome {
    pub fn none_could_see(&self) -> Option<String> {
        if self.found.is_some() || self.tried.is_empty() {
            return None;
        }
        let who: Vec<String> = self
            .tried
            .iter()
            .map(|p| format!("{} on {}", p.login, p.host))
            .collect();
        Some(format!("no account could see it, tried {}", who.join(", ")))
    }
}

pub fn find_account(
    programs: &Programs,
    located: &Located,
    registered: &[ForgeAccount],
    detected: &[Detected],
) -> Outcome {
    let mut outcome = Outcome::default();
    for (account, listed) in candidates(located, registered, detected) {
        outcome.tried.push(account.clone());
        if can_see(programs, located, &account) {
            outcome.found = Some(Found {
                account,
                registered: listed,
            });
            break;
        }
    }
    outcome
}

fn candidates(
    located: &Located,
    registered: &[ForgeAccount],
    detected: &[Detected],
) -> Vec<(Picked, bool)> {
    let mut list: Vec<(Picked, bool)> = registered
        .iter()
        .filter(|a| a.forge == located.forge && a.host == located.host)
        .map(|a| (picked(&a.login, &a.host), true))
        .collect();
    for d in detected
        .iter()
        .filter(|d| d.forge == located.forge && d.host.eq_ignore_ascii_case(&located.host))
    {
        let known = list
            .iter()
            .any(|(p, _)| p.login.eq_ignore_ascii_case(&d.login));
        if !known {
            list.push((picked(&d.login, &located.host), false));
        }
    }
    list
}

fn picked(login: &str, host: &str) -> Picked {
    Picked {
        login: login.to_string(),
        host: host.to_string(),
    }
}

fn can_see(programs: &Programs, located: &Located, account: &Picked) -> bool {
    let dir = std::env::temp_dir();
    let (program, session, args) = match located.forge {
        ForgeKind::Github => {
            let session = github_session(&programs.gh, account, &dir);
            let api = format!("repos/{}", located.path);
            let args = ["api", "--hostname", located.host.as_str(), api.as_str()];
            (&programs.gh, session, args.map(str::to_string).to_vec())
        }
        ForgeKind::Gitlab => {
            let session = gitlab_session(&programs.glab, account, &dir);
            let api = format!("projects/{}", encode(&located.path));
            let args = ["api", api.as_str(), "--hostname", located.host.as_str()];
            (&programs.glab, session, args.map(str::to_string).to_vec())
        }
    };
    let Ok(session) = session else {
        return false;
    };
    let mut envs: Vec<(&str, &str)> = session
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    envs.push(("GH_PROMPT_DISABLED", "1"));
    envs.push(("NO_COLOR", "1"));
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    matches!(
        capture_env(program, &args, &envs, PROBE_TIMEOUT),
        Ok(Some(out)) if out.success
    )
}

fn encode(path: &str) -> String {
    path.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                char::from(b).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(url: &str, gitlab_hosts: &[&str]) -> Option<(ForgeKind, String, String)> {
        let hosts: Vec<String> = gitlab_hosts.iter().map(|h| h.to_string()).collect();
        locate(url, &hosts).map(|l| (l.forge, l.host, l.path))
    }

    fn found(forge: ForgeKind, host: &str, path: &str) -> Option<(ForgeKind, String, String)> {
        Some((forge, host.to_string(), path.to_string()))
    }

    #[test]
    fn github_in_ssh_and_https_forms() {
        for url in [
            "https://github.com/acme/shop",
            "https://github.com/acme/shop.git",
            "https://github.com/acme/shop/",
            "git@github.com:acme/shop.git",
            "ssh://git@github.com:22/acme/shop.git",
            " https://GitHub.com/acme/shop ",
        ] {
            assert_eq!(
                at(url, &[]),
                found(ForgeKind::Github, "github.com", "acme/shop"),
                "{url}"
            );
        }
    }

    #[test]
    fn gitlab_keeps_nested_groups() {
        assert_eq!(
            at("git@gitlab.example.com:team/sub/app.git", &[]),
            found(ForgeKind::Gitlab, "gitlab.example.com", "team/sub/app")
        );
        assert_eq!(
            at("https://gitlab.com/a/b/c/d.git", &[]),
            found(ForgeKind::Gitlab, "gitlab.com", "a/b/c/d")
        );
    }

    #[test]
    fn a_host_glab_is_configured_for_is_gitlab_whatever_its_name() {
        let url = "https://code.example.com/g/app";
        assert_eq!(at(url, &[]), None);
        assert_eq!(
            at(url, &["Code.Example.com"]),
            found(ForgeKind::Gitlab, "code.example.com", "g/app")
        );
    }

    #[test]
    fn only_github_com_is_github() {
        assert_eq!(at("https://github.mycorp.com/a/b", &[]), None);
    }

    #[test]
    fn local_and_unparseable_urls_have_no_forge() {
        for url in ["file:///srv/x.git", "/srv/x", "https://github.com/acme", ""] {
            assert_eq!(at(url, &[]), None, "{url}");
        }
    }
}
