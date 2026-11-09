use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Error, Result};

pub const MAX_ACCOUNTS: usize = 50;
const MAX_LABEL_CHARS: usize = 64;
const MAX_LOGIN_CHARS: usize = 255;
const MAX_HOST_CHARS: usize = 253;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ForgeKind {
    Github,
    Gitlab,
}

impl ForgeKind {
    pub fn default_host(self) -> &'static str {
        match self {
            Self::Github => "github.com",
            Self::Gitlab => "gitlab.com",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Github => "github",
            Self::Gitlab => "gitlab",
        }
    }
}

/// One login on one forge host, as registered in the `forgeAccounts` setting. No token:
/// those are fetched from `gh` or `glab` when a child is spawned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgeAccount {
    pub forge: ForgeKind,
    pub login: String,
    /// Lowercase; `github.com` or `gitlab.com` unless the account is on another host.
    pub host: String,
    /// What the UI shows; the login when none was given.
    pub label: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Given {
    forge: ForgeKind,
    login: String,
    #[serde(default)]
    host: Option<String>,
    #[serde(default)]
    label: Option<String>,
}

fn invalid(message: String) -> Error {
    Error::InvalidInput(message)
}

fn check_login(forge: ForgeKind, login: &str) -> Result<()> {
    let shape = match forge {
        ForgeKind::Github => login.len() <= 39 && !login.contains(['.', '_']),
        ForgeKind::Gitlab => login.len() <= MAX_LOGIN_CHARS,
    };
    let ok = shape
        && !login.is_empty()
        && !login.starts_with('-')
        && login
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_'));
    if ok {
        Ok(())
    } else {
        Err(invalid(format!(
            "`{login}` is not a {} login",
            forge.as_str()
        )))
    }
}

fn check_host(host: &str) -> Result<()> {
    let name = host.split_once(':').map_or(host, |(name, port)| {
        if port.parse::<u16>().is_ok() {
            name
        } else {
            ""
        }
    });
    let ok = !name.is_empty()
        && host.len() <= MAX_HOST_CHARS
        && !name.starts_with(['-', '.'])
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.'));
    if ok {
        Ok(())
    } else {
        Err(invalid(format!(
            "`{host}` is not a host name (no scheme or path, an optional port)"
        )))
    }
}

fn account(given: Given) -> Result<ForgeAccount> {
    let login = given.login.trim().to_string();
    check_login(given.forge, &login)?;
    let host = match given.host {
        Some(host) => host.trim().to_ascii_lowercase(),
        None => given.forge.default_host().to_string(),
    };
    check_host(&host)?;
    let label = given
        .label
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .unwrap_or_else(|| login.clone());
    if label.chars().count() > MAX_LABEL_CHARS || label.chars().any(char::is_control) {
        return Err(invalid(format!(
            "an account label is at most {MAX_LABEL_CHARS} characters, on one line"
        )));
    }
    Ok(ForgeAccount {
        forge: given.forge,
        login,
        host,
        label,
    })
}

/// The accounts a `forgeAccounts` value says, with defaults filled in, or why it is not valid.
pub fn normalize(value: &Value) -> Result<Vec<ForgeAccount>> {
    let items = value
        .as_array()
        .ok_or_else(|| invalid("`forgeAccounts` is a list of accounts".into()))?;
    if items.len() > MAX_ACCOUNTS {
        return Err(invalid(format!(
            "at most {MAX_ACCOUNTS} forge accounts can be registered"
        )));
    }
    let mut accounts: Vec<ForgeAccount> = Vec::with_capacity(items.len());
    for item in items {
        let given: Given = serde_json::from_value(item.clone())
            .map_err(|e| invalid(format!("invalid forge account {item}: {e}")))?;
        let next = account(given)?;
        let same = |a: &&ForgeAccount| {
            (a.forge, &a.host) == (next.forge, &next.host)
                && a.login.eq_ignore_ascii_case(&next.login)
        };
        if accounts.iter().any(|a| same(&a)) {
            return Err(invalid(format!(
                "{} account `{}` on {} is listed twice",
                next.forge.as_str(),
                next.login,
                next.host
            )));
        }
        accounts.push(next);
    }
    Ok(accounts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_missing_host_and_label_take_their_defaults() {
        let got = normalize(&json!([
            {"forge": "github", "login": "work-me"},
            {"forge": "gitlab", "login": "me", "label": "Work GitLab"},
            {"forge": "gitlab", "login": "me", "host": "GitLab.Corp.io"}
        ]))
        .unwrap();
        let shown: Vec<_> = got
            .iter()
            .map(|a| (a.forge, a.login.as_str(), a.host.as_str(), a.label.as_str()))
            .collect();
        assert_eq!(
            shown,
            vec![
                (ForgeKind::Github, "work-me", "github.com", "work-me"),
                (ForgeKind::Gitlab, "me", "gitlab.com", "Work GitLab"),
                (ForgeKind::Gitlab, "me", "gitlab.corp.io", "me"),
            ]
        );
    }

    #[test]
    fn the_wire_form_is_camel_case_with_every_field() {
        let list = normalize(&json!([{"forge": "github", "login": "a"}])).unwrap();
        assert_eq!(
            serde_json::to_value(&list).unwrap(),
            json!([{"forge": "github", "login": "a", "host": "github.com", "label": "a"}])
        );
    }

    #[test]
    fn bad_accounts_are_refused() {
        let bad = [
            json!("a"),
            json!([7]),
            json!([{"forge": "bitbucket", "login": "a"}]),
            json!([{"forge": "github"}]),
            json!([{"forge": "github", "login": ""}]),
            json!([{"forge": "github", "login": "-lead"}]),
            json!([{"forge": "github", "login": "two words"}]),
            json!([{"forge": "gitlab", "login": "a/b"}]),
            json!([{"forge": "github", "login": "a", "host": "https://github.com"}]),
            json!([{"forge": "github", "login": "a", "host": "a b"}]),
            json!([{"forge": "github", "login": "a", "host": ""}]),
            json!([{"forge": "github", "login": "a", "label": "x".repeat(65)}]),
            json!([{"forge": "github", "login": "a", "token": "ghp_x"}]),
        ];
        for value in bad {
            assert!(normalize(&value).is_err(), "{value}");
        }
    }

    #[test]
    fn the_same_account_twice_is_refused_but_other_hosts_and_forges_are_fine() {
        let twice = json!([
            {"forge": "github", "login": "Me"},
            {"forge": "github", "login": "me", "host": "GITHUB.com"}
        ]);
        let err = normalize(&twice).unwrap_err().to_string();
        assert!(err.contains("twice"), "{err}");
        let fine = json!([
            {"forge": "github", "login": "me"},
            {"forge": "github", "login": "me", "host": "github.corp.io"},
            {"forge": "gitlab", "login": "me"}
        ]);
        assert_eq!(normalize(&fine).unwrap().len(), 3);
    }

    #[test]
    fn there_is_a_limit() {
        let many: Vec<_> = (0..=MAX_ACCOUNTS)
            .map(|i| json!({"forge": "github", "login": format!("u{i}")}))
            .collect();
        assert!(normalize(&json!(many)).is_err());
        assert!(normalize(&json!(many[..MAX_ACCOUNTS])).is_ok());
        assert_eq!(normalize(&json!([])).unwrap(), vec![]);
    }
}
