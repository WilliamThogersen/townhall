//! `marketplace.json` (section 2.2): the index of a marketplace and where each plugin lives.

use serde::de::Error as _;
use serde::{Deserialize, Deserializer};
use serde_json::Value;

use super::check;
use super::manifest::{validate_links, Author, Permissions};
use crate::Result;

pub const MAX_MARKETPLACE_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Owner {
    pub name: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}

pub(super) fn path_list<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Vec<String>, D::Error> {
    match Value::deserialize(deserializer)? {
        Value::String(path) => Ok(vec![path]),
        Value::Array(items) => items
            .into_iter()
            .map(|item| match item {
                Value::String(path) => Ok(path),
                _ => Err(D::Error::custom("must be a path or a list of paths")),
            })
            .collect(),
        _ => Err(D::Error::custom("must be a path or a list of paths")),
    }
}

pub(super) fn check_paths(what: &str, paths: &[String]) -> Result<()> {
    for path in paths {
        let inside = path.strip_prefix("./").unwrap_or(path);
        check::rel_path(what, inside)?;
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metadata {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    Path(String),
    Github {
        repo: String,
        git_ref: Option<String>,
        sha: Option<String>,
    },
    Url {
        url: String,
        git_ref: Option<String>,
        sha: Option<String>,
    },
    GitSubdir {
        url: String,
        path: String,
        git_ref: Option<String>,
        sha: Option<String>,
    },
    Unsupported(String),
}

const UNSUPPORTED_KINDS: [&str; 1] = ["command"];

#[derive(Deserialize)]
#[serde(tag = "source", rename_all = "kebab-case", deny_unknown_fields)]
enum Tagged {
    Github {
        repo: String,
        #[serde(rename = "ref", default)]
        git_ref: Option<String>,
        #[serde(default)]
        sha: Option<String>,
    },
    Url {
        url: String,
        #[serde(default)]
        path: Option<String>,
        #[serde(rename = "ref", default)]
        git_ref: Option<String>,
        #[serde(default)]
        sha: Option<String>,
    },
    GitSubdir {
        url: String,
        path: String,
        #[serde(rename = "ref", default)]
        git_ref: Option<String>,
        #[serde(default)]
        sha: Option<String>,
    },
}

impl<'de> Deserialize<'de> for Source {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        match Value::deserialize(deserializer)? {
            Value::String(path) => Ok(Source::Path(path)),
            Value::Object(map)
                if map
                    .get("source")
                    .and_then(Value::as_str)
                    .is_some_and(|kind| UNSUPPORTED_KINDS.contains(&kind)) =>
            {
                let kind = map["source"].as_str().unwrap_or_default();
                Ok(Source::Unsupported(kind.to_string()))
            }
            object @ Value::Object(_) => {
                let tagged = serde_json::from_value::<Tagged>(object).map_err(D::Error::custom)?;
                Ok(match tagged {
                    Tagged::Github { repo, git_ref, sha } => Source::Github { repo, git_ref, sha },
                    Tagged::Url {
                        url,
                        path: Some(path),
                        git_ref,
                        sha,
                    } => Source::GitSubdir {
                        url,
                        path,
                        git_ref,
                        sha,
                    },
                    Tagged::Url {
                        url,
                        path: None,
                        git_ref,
                        sha,
                    } => Source::Url { url, git_ref, sha },
                    Tagged::GitSubdir {
                        url,
                        path,
                        git_ref,
                        sha,
                    } => Source::GitSubdir {
                        url,
                        path,
                        git_ref,
                        sha,
                    },
                })
            }
            _ => Err(D::Error::custom(
                "source must be a `./path` string or an object with a `source` kind",
            )),
        }
    }
}

impl Source {
    fn validate(&self) -> Result<()> {
        match self {
            Source::Path(path) => {
                let Some(inside) = path.strip_prefix("./") else {
                    return Err(check::invalid(
                        "source",
                        format!(
                            "`{path}` must start with `./` and name a folder of the marketplace"
                        ),
                    ));
                };
                if !inside.is_empty() {
                    check::rel_path("source", inside)?;
                }
                Ok(())
            }
            Source::Github { repo, git_ref, sha } => {
                check::github_repo("repo", repo)?;
                pin(git_ref, sha)
            }
            Source::Url { url, git_ref, sha } => {
                check::git_url("url", url)?;
                pin(git_ref, sha)
            }
            Source::GitSubdir {
                url,
                path,
                git_ref,
                sha,
            } => {
                check::git_url("url", url)?;
                check::rel_path("path", path)?;
                pin(git_ref, sha)
            }
            Source::Unsupported(_) => Ok(()),
        }
    }
}

fn pin(git_ref: &Option<String>, sha: &Option<String>) -> Result<()> {
    if let Some(git_ref) = git_ref {
        check::git_ref("ref", git_ref)?;
    }
    if let Some(sha) = sha {
        check::sha("sha", sha)?;
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TownhallEntry {
    pub api_version: u32,
    #[serde(default)]
    pub permissions: Permissions,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub name: String,
    #[serde(default, rename = "displayName")]
    pub display_name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub author: Option<Author>,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub repository: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub strict: Option<bool>,
    #[serde(default, deserialize_with = "path_list")]
    pub skills: Vec<String>,
    #[serde(default, deserialize_with = "path_list")]
    pub commands: Vec<String>,
    #[serde(default, rename = "lspServers")]
    pub lsp_servers: Option<Value>,
    pub source: Source,
    #[serde(default)]
    pub townhall: Option<TownhallEntry>,
}

impl Entry {
    fn validate(&self) -> Result<()> {
        check::name("plugin name", &self.name)?;
        if let Some(version) = &self.version {
            check::semver("version", version)?;
        }
        validate_links(
            self.author.as_ref(),
            self.homepage.as_deref(),
            self.repository.as_deref(),
        )?;
        if let Some(townhall) = &self.townhall {
            if townhall.api_version == 0 {
                return Err(check::invalid("apiVersion", "must be 1 or higher"));
            }
            townhall.permissions.validate()?;
        }
        check_paths("skills", &self.skills)?;
        check_paths("commands", &self.commands)?;
        self.source.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Marketplace {
    #[serde(rename = "$schema", default)]
    pub schema: Option<String>,
    pub name: String,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub owner: Owner,
    #[serde(default)]
    pub metadata: Option<Metadata>,
    #[serde(default)]
    pub renames: Option<Value>,
    pub plugins: Vec<Entry>,
}

pub fn parse(text: &str) -> Result<Marketplace> {
    check::size("marketplace.json", text, MAX_MARKETPLACE_BYTES)?;
    let market: Marketplace =
        serde_json::from_str(text).map_err(|e| check::invalid("marketplace.json", e))?;
    market.validate()?;
    Ok(market)
}

impl Marketplace {
    fn validate(&self) -> Result<()> {
        check::name("marketplace name", &self.name)?;
        if let Some(url) = &self.owner.url {
            check::http_url("owner.url", url)?;
        }
        if let Some(version) = self.metadata.as_ref().and_then(|m| m.version.as_deref()) {
            check::semver("metadata.version", version)?;
        }
        for (i, entry) in self.plugins.iter().enumerate() {
            entry.validate()?;
            let earlier = &self.plugins[..i];
            if earlier
                .iter()
                .any(|other| other.name.eq_ignore_ascii_case(&entry.name))
            {
                return Err(check::invalid(
                    "plugins",
                    format!(
                        "`{}` is listed twice (names are unique ignoring case)",
                        entry.name
                    ),
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
