//! Which registered account a repo's children (`gh`, `glab`, `git push`) run as.

use crate::model::{ForgeAccount, ForgeKind, Repo, Settings};
use crate::{Error, Result};

/// The login and host to run as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picked {
    pub login: String,
    pub host: String,
}

/// Everything that decides the account: the registered list, the repo's own picks and the
/// old `githubAccount` setting.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Accounts {
    pub registered: Vec<ForgeAccount>,
    pub github: Option<String>,
    pub gitlab: Option<String>,
    pub legacy_github: String,
}

impl Accounts {
    pub fn for_repo(settings: &Settings, repo: Option<&Repo>) -> Self {
        Self {
            registered: settings.forge_accounts.clone(),
            github: repo.and_then(|r| r.github_account.clone()),
            gitlab: repo.and_then(|r| r.gitlab_account.clone()),
            legacy_github: settings.github_account.clone(),
        }
    }

    /// The repo's own pick when it has one (an error when that account is not registered for
    /// the remote's host), else the first registered account for the forge and host, else for
    /// GitHub the old `githubAccount`. `None` runs the children as whoever is logged in.
    pub fn pick(&self, forge: ForgeKind, host: &str) -> Result<Option<Picked>> {
        let host = host.to_ascii_lowercase();
        let here = |a: &&ForgeAccount| a.forge == forge && a.host == host;
        let chosen = match forge {
            ForgeKind::Github => &self.github,
            ForgeKind::Gitlab => &self.gitlab,
        };
        if let Some(want) = chosen {
            let found = self
                .registered
                .iter()
                .filter(here)
                .find(|a| a.login.eq_ignore_ascii_case(want));
            return match found {
                Some(a) => Ok(Some(picked(a))),
                None => Err(Error::InvalidInput(format!(
                    "this repo uses the {} account `{want}`, but no such account is registered for {host}: add it in settings or pick another one for the repo",
                    forge.as_str()
                ))),
            };
        }
        if let Some(first) = self.registered.iter().find(here) {
            return Ok(Some(picked(first)));
        }
        let legacy = self.legacy_github.trim();
        if forge == ForgeKind::Github && !legacy.is_empty() {
            return Ok(Some(Picked {
                login: legacy.to_string(),
                host,
            }));
        }
        Ok(None)
    }
}

fn picked(account: &ForgeAccount) -> Picked {
    Picked {
        login: account.login.clone(),
        host: account.host.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn registered(list: serde_json::Value) -> Vec<ForgeAccount> {
        crate::model::normalize_accounts(&list).unwrap()
    }

    fn accounts(github: Option<&str>, gitlab: Option<&str>, legacy: &str) -> Accounts {
        Accounts {
            registered: registered(json!([
                {"forge": "github", "login": "work-me"},
                {"forge": "github", "login": "private-me"},
                {"forge": "github", "login": "corp-me", "host": "github.corp.io"},
                {"forge": "gitlab", "login": "gl-work", "host": "gitlab.corp.io"}
            ])),
            github: github.map(str::to_string),
            gitlab: gitlab.map(str::to_string),
            legacy_github: legacy.to_string(),
        }
    }

    fn login(got: Result<Option<Picked>>) -> Option<(String, String)> {
        got.unwrap().map(|p| (p.login, p.host))
    }

    fn own(login: &str, host: &str) -> Option<(String, String)> {
        Some((login.to_string(), host.to_string()))
    }

    #[test]
    fn a_repo_without_a_pick_uses_the_first_account_for_its_forge_and_host() {
        let a = accounts(None, None, "");
        let github = a.pick(ForgeKind::Github, "github.com");
        assert_eq!(login(github), own("work-me", "github.com"));
        let corp = a.pick(ForgeKind::Github, "github.corp.io");
        assert_eq!(login(corp), own("corp-me", "github.corp.io"));
        let gitlab = a.pick(ForgeKind::Gitlab, "gitlab.corp.io");
        assert_eq!(login(gitlab), own("gl-work", "gitlab.corp.io"));
    }

    #[test]
    fn the_repos_own_pick_wins_ignoring_case() {
        let a = accounts(Some("Private-Me"), None, "");
        let got = a.pick(ForgeKind::Github, "github.com");
        assert_eq!(login(got), own("private-me", "github.com"));
    }

    #[test]
    fn a_pick_that_is_not_registered_for_the_remotes_host_is_an_error_that_says_so() {
        let a = accounts(Some("private-me"), None, "");
        let err = a
            .pick(ForgeKind::Github, "github.corp.io")
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("private-me") && err.contains("github.corp.io"),
            "{err}"
        );
        let gone = accounts(Some("deleted"), None, "");
        assert!(gone.pick(ForgeKind::Github, "github.com").is_err());
    }

    #[test]
    fn the_old_github_account_is_the_fallback_for_github_only() {
        let a = Accounts {
            registered: vec![],
            github: None,
            gitlab: None,
            legacy_github: "WilliamThogersen".into(),
        };
        let got = a.pick(ForgeKind::Github, "github.com");
        assert_eq!(login(got), own("WilliamThogersen", "github.com"));
        let other = a.pick(ForgeKind::Github, "github.corp.io");
        assert_eq!(login(other), own("WilliamThogersen", "github.corp.io"));
        assert_eq!(login(a.pick(ForgeKind::Gitlab, "gitlab.com")), None);
    }

    #[test]
    fn a_registered_account_beats_the_old_setting_and_nothing_means_no_account() {
        let a = accounts(None, None, "WilliamThogersen");
        let got = a.pick(ForgeKind::Github, "github.com");
        assert_eq!(login(got), own("work-me", "github.com"));
        let none = accounts(None, None, "");
        assert_eq!(login(none.pick(ForgeKind::Gitlab, "gitlab.com")), None);
        let empty = Accounts {
            registered: vec![],
            github: None,
            gitlab: None,
            legacy_github: String::new(),
        };
        assert_eq!(login(empty.pick(ForgeKind::Github, "github.com")), None);
    }

    #[test]
    fn hosts_compare_ignoring_case() {
        let a = accounts(None, None, "");
        let got = a.pick(ForgeKind::Gitlab, "GitLab.Corp.io");
        assert_eq!(login(got), own("gl-work", "gitlab.corp.io"));
    }
}
