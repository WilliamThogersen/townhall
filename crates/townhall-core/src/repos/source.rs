//! What someone typed into "add repository": a folder on this machine or a git remote.

use super::names;
use crate::{Error, Result};

const SCHEMES: [&str; 5] = ["https", "http", "ssh", "git", "file"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// Anything that is not a URL: handed to `resolve_path`.
    Path(String),
    Remote(Remote),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Remote {
    /// What `git clone` gets: the input without trailing slashes.
    pub url: String,
    /// `host/org/repo` lowercased, no `.git`: the same repo under any spelling of its URL.
    pub identity: String,
    /// The last path segment without `.git`: the default repo name.
    pub name: String,
}

pub fn classify(input: &str) -> Result<Source> {
    let input = input.trim();
    if input.is_empty() {
        return Err(Error::InvalidInput("path or url is required".into()));
    }
    match split_remote(input) {
        Some((scheme, host, path)) => remote(input, scheme, host, path).map(Source::Remote),
        None => Ok(Source::Path(input.to_string())),
    }
}

/// `(scheme, host, path)` of a URL (`scheme://[user@]host[:port]/path`) or an scp-style
/// address (`user@host:path`); `None` for anything else, which is a folder.
fn split_remote(input: &str) -> Option<(&str, &str, &str)> {
    if let Some((scheme, rest)) = input.split_once("://") {
        let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
        let host = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
        let host = host.split_once(':').map_or(host, |(h, _)| h);
        return Some((scheme, host, path));
    }
    let (address, path) = input.split_once(':')?;
    let (_, host) = address.split_once('@')?;
    (!address.contains('/')).then_some(("ssh", host, path))
}

fn invalid(input: &str, why: &str) -> Error {
    Error::InvalidInput(format!("`{input}` is not a git url: {why}"))
}

fn remote(input: &str, scheme: &str, host: &str, path: &str) -> Result<Remote> {
    if input.starts_with('-') || input.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(invalid(input, "it has blanks or starts with `-`"));
    }
    let scheme = scheme.to_ascii_lowercase();
    if !SCHEMES.contains(&scheme.as_str()) {
        return Err(invalid(input, "use https, ssh, git or file"));
    }
    let local = scheme == "file";
    if host.is_empty() && !local {
        return Err(invalid(input, "the host is missing"));
    }
    let path = path.trim_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let segments: Vec<&str> = path.split('/').collect();
    let needed = if local { 1 } else { 2 };
    let bad_segment = |s: &&str| s.is_empty() || *s == "." || *s == "..";
    if segments.len() < needed || segments.iter().any(bad_segment) {
        return Err(invalid(input, "expected `host/owner/repo`"));
    }
    let identity = format!("{host}/{path}");
    let name = segments.last().copied().unwrap_or_default();
    Ok(Remote {
        url: input.trim_end_matches('/').to_string(),
        identity: if local {
            identity
        } else {
            identity.to_ascii_lowercase()
        },
        name: names::from_folder(std::path::Path::new(name)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn remote(input: &str) -> Remote {
        match classify(input).unwrap() {
            Source::Remote(r) => r,
            other => panic!("{input} was {other:?}"),
        }
    }

    #[test]
    fn paths_stay_paths() {
        for input in ["/x/shop", "~/shop", "~", "relative/x", "C:/x", "/a@b:c/x"] {
            assert_eq!(
                classify(input).unwrap(),
                Source::Path(input.into()),
                "{input}"
            );
        }
        assert_eq!(classify("  /x ").unwrap(), Source::Path("/x".into()));
        assert!(classify("   ").is_err());
    }

    #[test]
    fn github_and_gitlab_links_in_every_spelling() {
        let spellings = [
            "https://github.com/acme/shop",
            "https://github.com/acme/shop.git",
            "https://github.com/acme/shop/",
            "https://github.com/acme/shop.git/",
            " https://GitHub.com/Acme/Shop ",
            "git@github.com:acme/shop.git",
            "git@github.com:acme/shop",
            "git@github.com:acme/shop/",
            "ssh://git@github.com/acme/shop.git",
            "ssh://git@github.com:22/acme/shop.git",
        ];
        for input in spellings {
            let r = remote(input);
            assert_eq!(r.identity, "github.com/acme/shop", "{input}");
            assert!(r.name.eq_ignore_ascii_case("shop"), "{input}");
            assert!(!r.url.ends_with('/'), "{input}");
            assert_eq!(r.url, r.url.trim(), "{input}");
        }
    }

    #[test]
    fn gitlab_subgroups_keep_every_segment() {
        let r = remote("https://gitlab.com/group/sub/team/app.git");
        assert_eq!(r.identity, "gitlab.com/group/sub/team/app");
        assert_eq!(r.name, "app");
        let r = remote("git@gitlab.com:group/sub/app.git");
        assert_eq!(r.identity, "gitlab.com/group/sub/app");
    }

    #[test]
    fn a_file_url_is_a_remote_too() {
        let r = remote("file:///tmp/x/origin.git");
        assert_eq!(r.name, "origin");
        assert_eq!(r.identity, "/tmp/x/origin");
    }

    #[test]
    fn urls_that_cannot_name_a_repo_are_refused() {
        for bad in [
            "https://github.com",
            "https://github.com/",
            "https://github.com/acme",
            "git@github.com:",
            "git@github.com:shop",
            "ftp://github.com/acme/shop",
            "ext::sh -c x://a/b",
            "https:///acme/shop",
            "https://github.com/acme/..",
        ] {
            let got = classify(bad);
            assert!(
                matches!(got, Err(Error::InvalidInput(_))),
                "{bad}: {got:?}"
            );
        }
    }
}
