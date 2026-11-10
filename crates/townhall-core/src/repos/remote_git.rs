use std::path::Path;

use base64::Engine;

use super::accounts::Accounts;
use super::auth_env::{auth_failure, remote_env};
use super::clone::clone_into_with;
use super::publish::{token_for, Programs};
use super::source::{split_remote, Remote};
use crate::cmd::run_blocking_env;
use crate::model::{ForgeAccount, ForgeKind};
use crate::redact::redact;
use crate::{Error, Result};

pub const GIT_BIN_ENV: &str = "TOWNHALL_GIT_BIN";

pub type AuthEnv = Vec<(String, String)>;

pub fn git_program() -> String {
    std::env::var(GIT_BIN_ENV)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "git".to_string())
}

fn forge_of(host: &str) -> Option<ForgeKind> {
    let host = host.to_ascii_lowercase();
    if host.contains("github") {
        Some(ForgeKind::Github)
    } else if host.contains("gitlab") {
        Some(ForgeKind::Gitlab)
    } else {
        None
    }
}

fn header(forge: ForgeKind, token: &str) -> String {
    match forge {
        ForgeKind::Github => {
            let basic =
                base64::engine::general_purpose::STANDARD.encode(format!("x-access-token:{token}"));
            format!("Authorization: Basic {basic}")
        }
        ForgeKind::Gitlab => format!("PRIVATE-TOKEN: {token}"),
    }
}

fn config_env(forge: ForgeKind, token: &str) -> AuthEnv {
    vec![
        ("GIT_CONFIG_COUNT".to_string(), "1".to_string()),
        (
            "GIT_CONFIG_KEY_0".to_string(),
            "http.extraheader".to_string(),
        ),
        ("GIT_CONFIG_VALUE_0".to_string(), header(forge, token)),
    ]
}

pub fn auth_env(
    programs: &Programs,
    url: &str,
    account: Option<&str>,
    registered: &[ForgeAccount],
    dir: &Path,
) -> Result<AuthEnv> {
    let Some((scheme, host, _)) = split_remote(url) else {
        return Ok(AuthEnv::new());
    };
    if !scheme.eq_ignore_ascii_case("https") {
        return Ok(AuthEnv::new());
    }
    let forge = match (forge_of(host), account) {
        (Some(forge), _) => forge,
        (None, Some(_)) => {
            return Err(Error::InvalidInput(format!(
                "cannot tell whether {host} is GitHub or GitLab: an account only applies to a GitHub or GitLab host"
            )))
        }
        (None, None) => return Ok(AuthEnv::new()),
    };
    let own = account.map(str::to_string);
    let accounts = Accounts {
        registered: registered.to_vec(),
        github: own.clone(),
        gitlab: own,
        legacy_github: String::new(),
    };
    match accounts.pick(forge, host)? {
        Some(picked) => Ok(config_env(
            forge,
            &token_for(programs, forge, &picked, dir)?,
        )),
        None => Ok(AuthEnv::new()),
    }
}

pub fn run(args: &[&str], cwd: Option<&Path>, auth: &AuthEnv) -> Result<String> {
    let mut envs: Vec<(&str, &str)> = remote_env();
    envs.push(("LC_MESSAGES", "C"));
    envs.extend(auth.iter().map(|(k, v)| (k.as_str(), v.as_str())));
    run_blocking_env(&git_program(), args, cwd, &envs)
}

pub fn clone(remote: &Remote, dest: &Path, auth: &AuthEnv) -> Result<()> {
    let mut envs: Vec<(&str, &str)> = vec![("LC_MESSAGES", "C")];
    envs.extend(auth.iter().map(|(k, v)| (k.as_str(), v.as_str())));
    clone_into_with(&git_program(), remote, dest, &envs)
}

pub fn failed(what: &str, url: &str, stderr: &str) -> Error {
    let https = url.to_ascii_lowercase().starts_with("http");
    let why = auth_failure(stderr, https).map_or_else(|| stderr.trim().to_string(), str::to_string);
    Error::InvalidInput(redact(&format!("could not {what} {url}: {why}")).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(env: &AuthEnv, key: &str) -> Option<String> {
        env.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone())
    }

    #[test]
    fn a_github_token_travels_as_a_basic_header_in_git_config_env() {
        let env = config_env(ForgeKind::Github, "tok-work-me");
        assert_eq!(value(&env, "GIT_CONFIG_COUNT").as_deref(), Some("1"));
        assert_eq!(
            value(&env, "GIT_CONFIG_KEY_0").as_deref(),
            Some("http.extraheader")
        );
        assert_eq!(
            value(&env, "GIT_CONFIG_VALUE_0").as_deref(),
            Some("Authorization: Basic eC1hY2Nlc3MtdG9rZW46dG9rLXdvcmstbWU=")
        );
    }

    #[test]
    fn a_gitlab_token_travels_as_a_private_token_header() {
        let env = config_env(ForgeKind::Gitlab, "glpat-x");
        assert_eq!(
            value(&env, "GIT_CONFIG_VALUE_0").as_deref(),
            Some("PRIVATE-TOKEN: glpat-x")
        );
    }

    #[test]
    fn hosts_are_told_apart_by_name() {
        assert_eq!(forge_of("github.corp.io"), Some(ForgeKind::Github));
        assert_eq!(forge_of("GitLab.example.com"), Some(ForgeKind::Gitlab));
        assert_eq!(forge_of("code.example.com"), None);
    }

    #[test]
    fn ssh_plain_http_and_unknown_hosts_get_no_token() {
        let programs = Programs {
            gh: "/no/such/gh".into(),
            glab: "/no/such/glab".into(),
        };
        let dir = Path::new("/");
        for url in [
            "git@github.com:acme/shop.git",
            "http://github.com/acme/shop.git",
            "https://code.example.com/acme/shop.git",
            "file:///srv/shop",
        ] {
            let env = auth_env(&programs, url, None, &[], dir).unwrap();
            assert!(env.is_empty(), "{url}: {env:?}");
        }
    }

    #[test]
    fn an_account_on_an_unknown_https_host_is_refused() {
        let programs = Programs {
            gh: "gh".into(),
            glab: "glab".into(),
        };
        let err = auth_env(
            &programs,
            "https://code.example.com/a/b.git",
            Some("me"),
            &[],
            Path::new("/"),
        )
        .unwrap_err();
        assert!(err.to_string().contains("GitHub or GitLab"), "{err}");
    }

    #[test]
    fn a_failure_keeps_git_s_reason_and_hides_nothing_useful() {
        let err = failed("fetch", "https://x/y", "fatal: Could not resolve host: x");
        assert!(err.to_string().contains("Could not resolve host"), "{err}");
        let err = failed("fetch", "https://x/y", "fatal: could not read Username");
        assert!(err.to_string().contains("authentication failed"), "{err}");
    }
}
