//! The accounts `gh` and `glab` are already logged in with, read from `auth status`, so the
//! settings page can offer them instead of making people type them. Only logins and hosts are
//! read: no token is requested, parsed or returned.

use std::io::{ErrorKind, Read};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{json, Value};

use super::auth_env::remote_env;
use super::publish::Programs;
use crate::model::{normalize_accounts, ForgeAccount, ForgeKind};
use crate::{Error, Result};

const STATUS_TIMEOUT: Duration = Duration::from_secs(15);

/// One login found on one host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Detected {
    pub forge: ForgeKind,
    pub login: String,
    pub host: String,
    /// Already in the `forgeAccounts` setting (same forge, host and login, ignoring case).
    pub registered: bool,
}

struct Output {
    stdout: String,
    stderr: String,
}

fn drain<R: Read + Send + 'static>(mut pipe: R) -> std::thread::JoinHandle<String> {
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = pipe.read_to_end(&mut bytes);
        String::from_utf8_lossy(&bytes).into_owned()
    })
}

/// Runs `program args` and keeps stdout and stderr, whatever the exit status: `auth status`
/// exits non-zero when any one account is broken. `None` when the program is not installed.
fn capture(program: &str, args: &[&str]) -> Result<Option<Output>> {
    let mut child = match Command::new(program)
        .args(args)
        .envs(remote_env())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(source) if source.kind() == ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(Error::Spawn {
                program: program.to_string(),
                source,
            })
        }
    };
    let stdout = child.stdout.take().map(drain);
    let stderr = child.stderr.take().map(drain);
    let deadline = Instant::now() + STATUS_TIMEOUT;
    loop {
        let waited = child.try_wait().map_err(|source| Error::Spawn {
            program: program.to_string(),
            source,
        })?;
        if waited.is_some() {
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Error::InvalidInput(format!(
                "`{program} {}` did not answer within {} seconds",
                args.join(" "),
                STATUS_TIMEOUT.as_secs()
            )));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let read = |handle: Option<std::thread::JoinHandle<String>>| {
        handle.and_then(|h| h.join().ok()).unwrap_or_default()
    };
    Ok(Some(Output {
        stdout: read(stdout),
        stderr: read(stderr),
    }))
}

/// `(host, login)` pairs from `gh auth status --json hosts`: `{"hosts": {host: [{state,
/// login, ...}]}}`. Accounts whose state is not `success` (an expired token) are left out.
/// `None` when the text is not that JSON, as on a gh that has no `--json`.
fn from_json(text: &str) -> Option<Vec<(String, String)>> {
    let value: Value = serde_json::from_str(text.trim()).ok()?;
    let hosts = value.get("hosts")?.as_object()?;
    let mut found = Vec::new();
    for (host, entries) in hosts {
        for entry in entries.as_array()? {
            if entry.get("state").and_then(Value::as_str) != Some("success") {
                continue;
            }
            if let Some(login) = entry.get("login").and_then(Value::as_str) {
                found.push((host.clone(), login.to_string()));
            }
        }
    }
    Some(found)
}

/// `(host, login)` pairs from the text of `auth status`, which both gh and glab print, to
/// stdout or stderr depending on the version: `Logged in to <host> account <login> (...)`
/// (gh 2.40 and later) or `Logged in to <host> as <login> (...)` (older gh, glab). Failed
/// logins read `Failed to log in`, so they never match.
fn from_text(text: &str) -> Vec<(String, String)> {
    const MARK: &str = "Logged in to ";
    text.lines()
        .filter_map(|line| {
            let rest = &line[line.find(MARK)? + MARK.len()..];
            let mut words = rest.split_whitespace();
            let host = words.next()?;
            let login = match words.next()? {
                "account" | "as" => words.next()?,
                _ => return None,
            };
            Some((host.to_string(), login.to_string()))
        })
        .collect()
}

fn found_by(program: &str, forge: ForgeKind) -> Result<Vec<(String, String)>> {
    if forge == ForgeKind::Github {
        let json = capture(program, &["auth", "status", "--json", "hosts"])?;
        match json {
            None => return Ok(vec![]),
            Some(out) => {
                if let Some(found) = from_json(&out.stdout) {
                    return Ok(found);
                }
            }
        }
    }
    let Some(out) = capture(program, &["auth", "status"])? else {
        return Ok(vec![]);
    };
    let mut found = from_text(&out.stdout);
    found.extend(from_text(&out.stderr));
    Ok(found)
}

/// Only what could be registered: a login or host the registry would refuse (a GitHub
/// enterprise managed user with an underscore, say) is left out, so "add" never fails.
fn accounts(forge: ForgeKind, pairs: Vec<(String, String)>) -> Vec<ForgeAccount> {
    let mut out: Vec<ForgeAccount> = Vec::new();
    for (host, login) in pairs {
        let one = json!([{"forge": forge, "login": login, "host": host}]);
        let Ok(mut parsed) = normalize_accounts(&one) else {
            continue;
        };
        let account = parsed.remove(0);
        let twice = out
            .iter()
            .any(|a| a.host == account.host && a.login.eq_ignore_ascii_case(&account.login));
        if !twice {
            out.push(account);
        }
    }
    out
}

fn mark(found: Vec<ForgeAccount>, registered: &[ForgeAccount]) -> Vec<Detected> {
    found
        .into_iter()
        .map(|a| Detected {
            registered: registered.iter().any(|r| {
                r.forge == a.forge && r.host == a.host && r.login.eq_ignore_ascii_case(&a.login)
            }),
            forge: a.forge,
            login: a.login,
            host: a.host,
        })
        .collect()
}

/// Every account `gh` and then `glab` report as logged in. A program that is not installed, or
/// has no login, adds nothing. Blocking: it runs both programs.
pub fn detect_blocking(programs: &Programs, registered: &[ForgeAccount]) -> Result<Vec<Detected>> {
    let gh = accounts(
        ForgeKind::Github,
        found_by(&programs.gh, ForgeKind::Github)?,
    );
    let glab = accounts(
        ForgeKind::Gitlab,
        found_by(&programs.glab, ForgeKind::Gitlab)?,
    );
    Ok(mark(gh.into_iter().chain(glab).collect(), registered))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
        list.iter()
            .map(|(h, l)| (h.to_string(), l.to_string()))
            .collect()
    }

    #[test]
    fn text_from_new_and_old_versions_is_read_and_failures_are_not() {
        let text = "github.com\n  ✓ Logged in to github.com account a (keyring)\n  - Token: gho_****\n  X Failed to log in to github.com account b (keyring)\n  ✓ Logged in to ghe.io as c (oauth_token)\nLogged in to nowhere\n";
        assert_eq!(
            from_text(text),
            pairs(&[("github.com", "a"), ("ghe.io", "c")])
        );
        assert_eq!(from_text("You are not logged into any hosts"), vec![]);
    }

    const REAL_GH_JSON: &str = r#"{"hosts":{"github.com":[{"state":"success","active":true,"host":"github.com","login":"WilliamThogersen","tokenSource":"keyring","scopes":"gist, read:org, repo, workflow","gitProtocol":"https"},{"state":"success","active":false,"host":"github.com","login":"wexowgt","tokenSource":"keyring","scopes":"gist, read:org, repo, workflow","gitProtocol":"https"}]}}"#;

    const REAL_GH_TEXT: &str = "github.com\n  ✓ Logged in to github.com account WilliamThogersen (keyring)\n  - Active account: true\n  - Git operations protocol: https\n\n  ✓ Logged in to github.com account wexowgt (keyring)\n  - Active account: false\n  - Git operations protocol: https\n";

    #[test]
    fn real_gh_2_101_output_yields_both_accounts_in_json_and_text() {
        let both = pairs(&[
            ("github.com", "WilliamThogersen"),
            ("github.com", "wexowgt"),
        ]);
        assert_eq!(from_json(REAL_GH_JSON), Some(both.clone()));
        assert_eq!(from_text(REAL_GH_TEXT), both);
    }

    #[test]
    fn glab_text_with_several_hosts_is_read() {
        let text = "gitlab.com\n  ✓ Logged in to gitlab.com as gl-me (/home/me/.config/glab-cli/config.yml)\n  ✓ Token found: **********\ngitlab.corp.io\n  ✓ Logged in to gitlab.corp.io as corp.me (config)\n";
        assert_eq!(
            from_text(text),
            pairs(&[("gitlab.com", "gl-me"), ("gitlab.corp.io", "corp.me")])
        );
    }

    #[test]
    fn json_keeps_working_accounts_only_and_rejects_other_shapes() {
        let ok =
            r#"{"hosts":{"h.io":[{"state":"success","login":"a"},{"state":"error","login":"b"}]}}"#;
        assert_eq!(from_json(ok), Some(pairs(&[("h.io", "a")])));
        assert_eq!(from_json("unknown flag: --json"), None);
        assert_eq!(from_json(r#"{"hosts":{"h.io":"x"}}"#), None);
        assert_eq!(from_json(r#"{"hosts":{}}"#), Some(vec![]));
    }

    #[test]
    fn logins_the_registry_would_refuse_are_left_out_and_repeats_are_dropped() {
        let got = accounts(
            ForgeKind::Github,
            pairs(&[
                ("github.com", "Me"),
                ("github.com", "me"),
                ("github.com", "emu_corp"),
            ]),
        );
        let shown: Vec<_> = got.iter().map(|a| a.login.as_str()).collect();
        assert_eq!(shown, vec!["Me"]);
    }
}
