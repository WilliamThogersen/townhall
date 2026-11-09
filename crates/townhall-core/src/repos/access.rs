use super::source::split_remote;
use crate::model::ForgeKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Located {
    pub forge: ForgeKind,
    pub host: String,
    pub path: String,
}

pub fn locate(url: &str, gitlab_hosts: &[String]) -> Option<Located> {
    let (scheme, host, path) = split_remote(url.trim())?;
    if scheme.eq_ignore_ascii_case("file") || host.is_empty() {
        return None;
    }
    let host = host.to_ascii_lowercase();
    let path = path.trim_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    if path.split('/').count() < 2 || path.split('/').any(str::is_empty) {
        return None;
    }
    let listed = gitlab_hosts.iter().any(|h| h.eq_ignore_ascii_case(&host));
    let forge = if host == ForgeKind::Github.default_host() {
        ForgeKind::Github
    } else if listed || host.contains("gitlab") {
        ForgeKind::Gitlab
    } else {
        return None;
    };
    Some(Located {
        forge,
        host,
        path: path.to_string(),
    })
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
            at("git@gitlab.wexo.io:team/sub/app.git", &[]),
            found(ForgeKind::Gitlab, "gitlab.wexo.io", "team/sub/app")
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
