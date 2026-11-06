//! Pushing a task branch and opening its pull request (GitHub) or merge request (GitLab).

use super::source::split_remote;
use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Forge {
    GitHub,
    GitLab,
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

pub fn number_from_url(url: &str) -> Option<u64> {
    let mut segments = url.split(['?', '#']).next()?.trim_end_matches('/').split('/');
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
            assert!(err.to_string().contains("is a local folder"), "{url}: {err}");
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
