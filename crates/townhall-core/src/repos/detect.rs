//! The accounts `gh` and `glab` are already logged in with, so the settings page can offer them
//! instead of making people type them. `gh` is asked with `auth status`; `glab` is read from its
//! config file, because `glab auth status` calls every configured host and hangs on one that is
//! unreachable. Only logins and hosts are read: no token is requested, parsed or returned.

use std::io::{ErrorKind, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{json, Value};

use super::auth_env::remote_env;
use super::publish::Programs;
use crate::model::{normalize_accounts, ForgeAccount, ForgeKind};
use crate::{Error, Result};

const STATUS_TIMEOUT: Duration = Duration::from_secs(10);
const VERSION_TIMEOUT: Duration = Duration::from_secs(5);
const USER_TIMEOUT: Duration = Duration::from_secs(4);

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

/// What detection found, and why a program gave nothing or less than expected.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Detection {
    pub accounts: Vec<Detected>,
    pub warnings: Vec<String>,
}

type Pairs = Vec<(String, String)>;

#[derive(Default)]
struct Probe {
    pairs: Pairs,
    warnings: Vec<String>,
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
fn capture(program: &str, args: &[&str], timeout: Duration) -> Result<Option<Output>> {
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
    let deadline = Instant::now() + timeout;
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
                timeout.as_secs()
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
fn from_json(text: &str) -> Option<Pairs> {
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

/// `(host, login)` pairs from the text of `auth status`, to stdout or stderr depending on the
/// version: `Logged in to <host> account <login> (...)` (gh 2.40 and later) or
/// `Logged in to <host> as <login> (...)` (older gh). Failed logins read `Failed to log in`,
/// so they never match.
fn from_text(text: &str) -> Pairs {
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

/// One `key: value` line of a YAML file. Enough YAML for the two config files, which are plain
/// nested maps; a `token` value is dropped on the spot so it can never be passed on.
struct Entry {
    indent: usize,
    key: String,
    value: Option<String>,
}

fn unquote(text: &str) -> &str {
    let text = text.trim();
    for quote in ['"', '\''] {
        if let Some(inner) = text
            .strip_prefix(quote)
            .and_then(|rest| rest.strip_suffix(quote))
        {
            return inner;
        }
    }
    text
}

fn entries(text: &str) -> Vec<Entry> {
    text.lines()
        .filter_map(|raw| {
            let line = raw.trim_end();
            let body = line.trim_start();
            if body.is_empty() || body.starts_with('#') || body.starts_with("- ") {
                return None;
            }
            let (key, value) = match body.split_once(": ") {
                Some((key, value)) => (key, value),
                None => (body.strip_suffix(':')?, ""),
            };
            let key = unquote(key).to_string();
            let value = value.split_once(" #").map_or(value, |(before, _)| before);
            let value = unquote(value);
            let value = (key != "token" && !value.is_empty()).then(|| value.to_string());
            Some(Entry {
                indent: line.len() - body.len(),
                key,
                value,
            })
        })
        .collect()
}

/// The entries at `indent`, each with the deeper entries nested under it.
fn blocks(list: &[Entry], indent: usize) -> Vec<(&Entry, &[Entry])> {
    let mut found = Vec::new();
    let mut at = 0;
    while at < list.len() {
        if list[at].indent == indent {
            let len = list[at + 1..]
                .iter()
                .take_while(|e| e.indent > indent)
                .count();
            found.push((&list[at], &list[at + 1..at + 1 + len]));
            at += len;
        }
        at += 1;
    }
    found
}

/// The indent of the shallowest entry: where a block's own keys sit.
fn own_indent(body: &[Entry]) -> usize {
    body.iter().map(|e| e.indent).min().unwrap_or(0)
}

/// `(host, login)` from gh's `hosts.yml`: every key under `users:` plus the active `user:`.
fn gh_hosts(text: &str) -> Pairs {
    let list = entries(text);
    let mut found = Pairs::new();
    for (host, body) in blocks(&list, 0) {
        for (key, nested) in blocks(body, own_indent(body)) {
            match (key.key.as_str(), &key.value) {
                ("user", Some(login)) => found.push((host.key.clone(), login.clone())),
                ("users", None) => {
                    let logins = blocks(nested, own_indent(nested));
                    found.extend(
                        logins
                            .into_iter()
                            .map(|(l, _)| (host.key.clone(), l.key.clone())),
                    );
                }
                _ => {}
            }
        }
    }
    found
}

/// `(host, user)` from glab's `config.yml`, under `hosts:`. `user` is `None` when the host has
/// no `user` key (an older glab, or a token login that never looked the user up).
fn glab_hosts(text: &str) -> Vec<(String, Option<String>)> {
    let list = entries(text);
    let Some((_, body)) = blocks(&list, 0).into_iter().find(|(e, _)| e.key == "hosts") else {
        return vec![];
    };
    blocks(body, own_indent(body))
        .into_iter()
        .map(|(host, nested)| {
            let user = blocks(nested, own_indent(nested))
                .into_iter()
                .find(|(e, _)| e.key == "user")
                .and_then(|(e, _)| e.value.clone());
            (host.key.clone(), user)
        })
        .collect()
}

fn env_dir(var: &str) -> Option<PathBuf> {
    std::env::var(var)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// Where a program keeps its config: `<own_var>`, else `$XDG_CONFIG_HOME/<name>`, else
/// `~/.config/<name>`, the same order both programs use.
fn config_dir(own_var: &str, name: &str) -> PathBuf {
    env_dir(own_var)
        .or_else(|| env_dir("XDG_CONFIG_HOME").map(|d| d.join(name)))
        .or_else(|| env_dir("HOME").map(|d| d.join(".config").join(name)))
        .unwrap_or_else(|| PathBuf::from(".config").join(name))
}

fn read_config(path: &Path, program: &str) -> std::result::Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| match e.kind() {
        ErrorKind::NotFound => format!(
            "no {program} config at {}: log in with `{program} auth login`",
            path.display()
        ),
        _ => format!("could not read {}: {e}", path.display()),
    })
}

fn not_found(program: &str) -> String {
    format!("`{program}` was not found on PATH, so its accounts cannot be listed")
}

fn gh_by_command(program: &str) -> Result<Option<Pairs>> {
    let Some(json) = capture(
        program,
        &["auth", "status", "--json", "hosts"],
        STATUS_TIMEOUT,
    )?
    else {
        return Ok(None);
    };
    if let Some(found) = from_json(&json.stdout) {
        return Ok(Some(found));
    }
    let Some(out) = capture(program, &["auth", "status"], STATUS_TIMEOUT)? else {
        return Ok(None);
    };
    let mut found = from_text(&out.stdout);
    found.extend(from_text(&out.stderr));
    Ok(Some(found))
}

/// `gh auth status` first; when it fails or lists nothing, `hosts.yml` in the config dir.
fn probe_gh(program: &str, config: &Path) -> Probe {
    let mut probe = Probe::default();
    match gh_by_command(program) {
        Ok(Some(pairs)) if !pairs.is_empty() => {
            probe.pairs = pairs;
            return probe;
        }
        Ok(Some(_)) => probe
            .warnings
            .push("`gh auth status` listed no logged-in account".to_string()),
        Ok(None) => {
            probe.warnings.push(not_found(program));
            return probe;
        }
        Err(e) => probe.warnings.push(e.to_string()),
    }
    let path = config.join("hosts.yml");
    match read_config(&path, "gh") {
        Ok(text) => {
            probe.pairs = gh_hosts(&text);
            if probe.pairs.is_empty() {
                probe
                    .warnings
                    .push(format!("{} lists no logged-in account", path.display()));
            }
        }
        Err(warning) => probe.warnings.push(warning),
    }
    probe
}

/// The user glab is logged in as on `host`, asked of the host with a short timeout.
fn glab_user(program: &str, host: &str) -> std::result::Result<String, String> {
    let out = match capture(program, &["api", "user", "--hostname", host], USER_TIMEOUT) {
        Ok(Some(out)) => out,
        Ok(None) => return Err(not_found(program)),
        Err(e) => return Err(e.to_string()),
    };
    let value: Value = serde_json::from_str(out.stdout.trim()).map_err(|_| {
        let reason = out.stderr.lines().next().unwrap_or("no JSON answer").trim();
        format!("`glab api user` gave no user: {reason}")
    })?;
    value
        .get("username")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "`glab api user` answer has no username".to_string())
}

/// glab's hosts and users from `config.yml`, never from `auth status`. A host with no `user` key
/// is asked with `glab api user`, so one slow host costs a few seconds, not the whole list.
fn probe_glab(program: &str, config: &Path) -> Probe {
    let mut probe = Probe::default();
    match capture(program, &["--version"], VERSION_TIMEOUT) {
        Ok(Some(_)) => {}
        Ok(None) => {
            probe.warnings.push(not_found(program));
            return probe;
        }
        Err(e) => {
            probe.warnings.push(e.to_string());
            return probe;
        }
    }
    let path = config.join("config.yml");
    let text = match read_config(&path, "glab") {
        Ok(text) => text,
        Err(warning) => {
            probe.warnings.push(warning);
            return probe;
        }
    };
    let hosts = glab_hosts(&text);
    if hosts.is_empty() {
        probe
            .warnings
            .push(format!("{} lists no glab host", path.display()));
    }
    for (host, user) in hosts {
        match user {
            Some(user) => probe.pairs.push((host, user)),
            None => match glab_user(program, &host) {
                Ok(user) => probe.pairs.push((host, user)),
                Err(why) => probe
                    .warnings
                    .push(format!("could not tell who is logged in on {host}: {why}")),
            },
        }
    }
    probe
}

/// Only what could be registered: a login or host the registry would refuse (a GitHub
/// enterprise managed user with an underscore, say) is left out, so "add" never fails.
fn accounts(forge: ForgeKind, pairs: Pairs) -> Vec<ForgeAccount> {
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

fn detect_in(
    programs: &Programs,
    gh_config: &Path,
    glab_config: &Path,
    registered: &[ForgeAccount],
) -> Detection {
    let (gh, glab) = std::thread::scope(|scope| {
        let glab = scope.spawn(|| probe_glab(&programs.glab, glab_config));
        let gh = probe_gh(&programs.gh, gh_config);
        let glab = glab.join().unwrap_or_else(|_| Probe {
            pairs: vec![],
            warnings: vec!["detecting glab accounts panicked".to_string()],
        });
        (gh, glab)
    });
    let mut found = Vec::new();
    let mut warnings = Vec::new();
    for (forge, name, probe) in [
        (ForgeKind::Github, "gh", gh),
        (ForgeKind::Gitlab, "glab", glab),
    ] {
        let got = accounts(forge, probe.pairs);
        if got.is_empty() && probe.warnings.is_empty() {
            warnings.push(format!("{name}: no account could be registered"));
        }
        warnings.extend(probe.warnings.into_iter().map(|w| format!("{name}: {w}")));
        found.extend(got);
    }
    Detection {
        accounts: mark(found, registered),
        warnings,
    }
}

/// Every account `gh` and `glab` are logged in with. Blocking, but both programs run at the same
/// time. A program that is missing, slow or has no login adds nothing and a warning saying why,
/// and never hides the other's accounts.
pub fn detect_blocking(programs: &Programs, registered: &[ForgeAccount]) -> Detection {
    detect_in(
        programs,
        &config_dir("GH_CONFIG_DIR", "gh"),
        &config_dir("GLAB_CONFIG_DIR", "glab-cli"),
        registered,
    )
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

    const GH_HOSTS: &str = "github.com:\n    users:\n        WilliamThogersen:\n        wexowgt:\n    git_protocol: https\n    user: WilliamThogersen\nghe.corp.io:\n    oauth_token: gho_secret\n    user: corp-me\n";

    #[test]
    fn gh_hosts_yml_lists_every_user_and_the_active_one_and_no_token() {
        let got = gh_hosts(GH_HOSTS);
        assert_eq!(
            got,
            pairs(&[
                ("github.com", "WilliamThogersen"),
                ("github.com", "wexowgt"),
                ("github.com", "WilliamThogersen"),
                ("ghe.corp.io", "corp-me"),
            ])
        );
        assert!(!format!("{got:?}").contains("gho_secret"));
    }

    const GLAB_CONFIG: &str = "# glab config\ngit_protocol: ssh\nglamour_style: dark\nhosts:\n    gitlab.wexo.io:\n        token: glpat-secret\n        api_host: gitlab.wexo.io\n        git_protocol: ssh\n        user: wexowgt\n    \"gitlab.corp.io:8443\":\n        token: !!null glpat-other\n        user: 'corp.me' # who\n    gitlab.com:\n        api_host: gitlab.com\neditor:\n";

    #[test]
    fn glab_config_gives_each_host_and_its_user_and_never_a_token() {
        let got = glab_hosts(GLAB_CONFIG);
        assert_eq!(
            got,
            vec![
                ("gitlab.wexo.io".to_string(), Some("wexowgt".to_string())),
                (
                    "gitlab.corp.io:8443".to_string(),
                    Some("corp.me".to_string())
                ),
                ("gitlab.com".to_string(), None),
            ]
        );
        assert!(!format!("{got:?}").contains("glpat"));
        assert_eq!(glab_hosts("git_protocol: ssh\n"), vec![]);
    }

    #[cfg(unix)]
    mod with_stubs {
        use super::*;
        use std::os::unix::fs::PermissionsExt;

        fn stub(dir: &Path, name: &str, body: &str) -> String {
            std::fs::create_dir_all(dir.join("bin")).unwrap();
            let path = dir.join("bin").join(name);
            std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
            path.to_string_lossy().into_owned()
        }

        fn config(dir: &Path, name: &str, text: &str) -> PathBuf {
            let at = dir.join(name);
            std::fs::create_dir_all(&at).unwrap();
            let file = if name == "gh" {
                "hosts.yml"
            } else {
                "config.yml"
            };
            std::fs::write(at.join(file), text).unwrap();
            at
        }

        fn logins(found: &Detection) -> Vec<String> {
            found
                .accounts
                .iter()
                .map(|d| format!("{:?} {}@{}", d.forge, d.login, d.host))
                .collect()
        }

        // The real machine: gh answers; glab's `auth status` would hang on gitlab.com, so the stub
        // fails loudly if it is ever called, and only the config file knows the user.
        const GLAB: &str = "case \"$1\" in --version) echo 'glab 1.50';; api) echo \"{\\\"username\\\":\\\"from-api-$4\\\"}\";; *) echo 'auth status must not be called' >&2; sleep 30;; esac";

        #[test]
        fn glab_account_comes_from_its_config_without_auth_status() {
            let dir = tempfile::tempdir().unwrap();
            let gh = stub(dir.path(), "gh", &format!("echo '{REAL_GH_JSON}'"));
            let glab = stub(dir.path(), "glab", GLAB);
            let gh_cfg = config(dir.path(), "gh", "");
            let glab_cfg = config(
                dir.path(),
                "glab-cli",
                "hosts:\n    gitlab.wexo.io:\n        token: x\n        user: wexowgt\n",
            );
            let started = Instant::now();
            let found = detect_in(&Programs { gh, glab }, &gh_cfg, &glab_cfg, &[]);
            assert!(started.elapsed() < Duration::from_secs(8));
            assert_eq!(
                logins(&found),
                vec![
                    "Github WilliamThogersen@github.com",
                    "Github wexowgt@github.com",
                    "Gitlab wexowgt@gitlab.wexo.io"
                ]
            );
            assert_eq!(found.warnings, Vec::<String>::new());
        }

        #[test]
        fn a_host_without_a_user_is_asked_with_glab_api_user() {
            let dir = tempfile::tempdir().unwrap();
            let gh = stub(dir.path(), "gh", "exit 1");
            let glab = stub(dir.path(), "glab", GLAB);
            let gh_cfg = dir.path().join("none");
            let glab_cfg = config(
                dir.path(),
                "glab-cli",
                "hosts:\n    gitlab.example.com:\n        api_host: gitlab.example.com\n",
            );
            let found = detect_in(&Programs { gh, glab }, &gh_cfg, &glab_cfg, &[]);
            assert_eq!(
                logins(&found),
                vec!["Gitlab from-api-gitlab.example.com@gitlab.example.com"]
            );
        }

        #[test]
        fn a_failing_gh_falls_back_to_hosts_yml_and_says_so() {
            let dir = tempfile::tempdir().unwrap();
            let gh = stub(dir.path(), "gh", "echo boom >&2; exit 1");
            let glab = stub(dir.path(), "glab", GLAB);
            let gh_cfg = config(dir.path(), "gh", GH_HOSTS);
            let glab_cfg = dir.path().join("none");
            let found = detect_in(&Programs { gh, glab }, &gh_cfg, &glab_cfg, &[]);
            assert!(logins(&found).contains(&"Github wexowgt@github.com".to_string()));
            assert!(logins(&found).iter().all(|l| !l.starts_with("Gitlab")));
            assert!(
                found
                    .warnings
                    .iter()
                    .any(|w| w.starts_with("gh: `gh auth status` listed no")),
                "{:?}",
                found.warnings
            );
            assert!(
                found
                    .warnings
                    .iter()
                    .any(|w| w.starts_with("glab: no glab config at")),
                "{:?}",
                found.warnings
            );
        }

        #[test]
        fn a_missing_program_and_a_missing_user_each_leave_a_warning() {
            let dir = tempfile::tempdir().unwrap();
            let glab = stub(
                dir.path(),
                "glab",
                "case \"$1\" in --version) echo v;; *) echo 'HTTP 401' >&2; exit 1;; esac",
            );
            let glab_cfg = config(
                dir.path(),
                "glab-cli",
                "hosts:\n    gitlab.io:\n        api_host: gitlab.io\n",
            );
            let programs = Programs {
                gh: "/no/such/gh".to_string(),
                glab,
            };
            let found = detect_in(&programs, &dir.path().join("none"), &glab_cfg, &[]);
            assert_eq!(found.accounts, vec![]);
            assert!(
                found
                    .warnings
                    .iter()
                    .any(|w| w.starts_with("gh: `/no/such/gh` was not found")),
                "{:?}",
                found.warnings
            );
            assert!(
                found.warnings.iter().any(|w| w
                    .contains("could not tell who is logged in on gitlab.io")
                    && w.contains("HTTP 401")),
                "{:?}",
                found.warnings
            );
        }

        #[test]
        fn registered_accounts_are_marked() {
            let dir = tempfile::tempdir().unwrap();
            let gh = stub(dir.path(), "gh", &format!("echo '{REAL_GH_JSON}'"));
            let glab = stub(dir.path(), "glab", GLAB);
            let registered = normalize_accounts(
                &json!([{"forge": "github", "login": "wexowgt", "host": "github.com"}]),
            )
            .unwrap();
            let found = detect_in(
                &Programs { gh, glab },
                &dir.path().join("a"),
                &dir.path().join("b"),
                &registered,
            );
            let marks: Vec<_> = found.accounts.iter().map(|d| d.registered).collect();
            assert_eq!(marks, vec![false, true]);
        }
    }
}
