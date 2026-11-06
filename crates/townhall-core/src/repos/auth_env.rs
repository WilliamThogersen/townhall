//! The environment for git, ssh, gh and glab children: the user's login-shell `PATH`,
//! `SSH_AUTH_SOCK` and `GIT_SSH_COMMAND`, so private repos work when townhall is launched
//! from Finder, plus the settings that keep every child from waiting on a prompt.

use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::sync::OnceLock;
use std::time::Duration;

use super::clone::{CLONE_ENV, SSH_NO_PROMPT};

const SHELL_TIMEOUT: Duration = Duration::from_secs(5);
const MARK: &str = "__townhall__";
const FALLBACK_DIRS: [&str; 2] = ["/opt/homebrew/bin", "/usr/local/bin"];

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) struct Vars {
    pub path: Option<String>,
    pub ssh_auth_sock: Option<String>,
    pub ssh_command: Option<String>,
}

fn non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

pub(super) fn parse_shell_output(output: &str) -> Vars {
    let find = |key: &str| {
        let prefix = format!("{MARK}{key}=");
        let found = output.lines().rev().find_map(|l| l.strip_prefix(&prefix));
        non_empty(found.map(str::to_string))
    };
    Vars {
        path: find("PATH"),
        ssh_auth_sock: find("SSH_AUTH_SOCK"),
        ssh_command: find("GIT_SSH_COMMAND"),
    }
}

fn shell_script() -> String {
    ["PATH", "SSH_AUTH_SOCK", "GIT_SSH_COMMAND"]
        .iter()
        .map(|k| format!("printf '%s\\n' \"{MARK}{k}=${k}\""))
        .collect::<Vec<_>>()
        .join("; ")
}

pub(super) fn login_shell_vars(shell: &str, timeout: Duration) -> Option<Vars> {
    let mut child = Command::new(shell)
        .args(["-l", "-i", "-c", &shell_script()])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take()?;
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stdout.read_to_end(&mut bytes);
        let _ = tx.send(bytes);
    });
    match rx.recv_timeout(timeout) {
        Ok(bytes) => {
            let _ = child.wait();
            Some(parse_shell_output(&String::from_utf8_lossy(&bytes)))
        }
        Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
            None
        }
    }
}

fn launchd_ssh_auth_sock() -> Option<String> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    let out = Command::new("launchctl")
        .args(["getenv", "SSH_AUTH_SOCK"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| non_empty(Some(String::from_utf8_lossy(&out.stdout).into_owned())))
        .flatten()
}

pub(super) fn merge_path(shell: Option<&str>, current: Option<&str>, extra: &[&str]) -> String {
    let mut dirs: Vec<&str> = Vec::new();
    let all = shell
        .into_iter()
        .chain(current)
        .flat_map(|p| p.split(':'))
        .chain(extra.iter().copied());
    for dir in all.filter(|d| !d.is_empty()) {
        if !dirs.contains(&dir) {
            dirs.push(dir);
        }
    }
    dirs.join(":")
}

pub(super) fn compose(
    process: &Vars,
    shell: &Vars,
    launchd_sock: Option<String>,
) -> Vec<(String, String)> {
    let mut envs: Vec<(String, String)> = CLONE_ENV
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let path = merge_path(
        shell.path.as_deref(),
        process.path.as_deref(),
        &FALLBACK_DIRS,
    );
    envs.push(("PATH".into(), path));
    let sock = process
        .ssh_auth_sock
        .clone()
        .or_else(|| shell.ssh_auth_sock.clone())
        .or(launchd_sock);
    if let Some(sock) = sock {
        envs.push(("SSH_AUTH_SOCK".into(), sock));
    }
    let ssh = process
        .ssh_command
        .clone()
        .or_else(|| shell.ssh_command.clone())
        .unwrap_or_else(|| SSH_NO_PROMPT.to_string());
    envs.push(("GIT_SSH_COMMAND".into(), ssh));
    envs
}

fn process_vars() -> Vars {
    let var = |k: &str| non_empty(std::env::var(k).ok());
    Vars {
        path: var("PATH"),
        ssh_auth_sock: var("SSH_AUTH_SOCK"),
        ssh_command: var("GIT_SSH_COMMAND"),
    }
}

fn resolve() -> Vec<(String, String)> {
    let process = process_vars();
    let shell_program = non_empty(std::env::var("SHELL").ok()).unwrap_or_else(|| "/bin/zsh".into());
    let shell = login_shell_vars(&shell_program, SHELL_TIMEOUT).unwrap_or_default();
    let launchd = if process.ssh_auth_sock.is_none() && shell.ssh_auth_sock.is_none() {
        launchd_ssh_auth_sock()
    } else {
        None
    };
    compose(&process, &shell, launchd)
}

/// The environment for any child that talks to a remote, resolved once per process.
pub(crate) fn remote_env() -> Vec<(&'static str, &'static str)> {
    static RESOLVED: OnceLock<Vec<(String, String)>> = OnceLock::new();
    RESOLVED
        .get_or_init(resolve)
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect()
}

pub(super) fn auth_failure(stderr: &str) -> Option<&'static str> {
    let lower = stderr.to_ascii_lowercase();
    let has = |needles: &[&str]| needles.iter().any(|n| lower.contains(n));
    if has(&["host key verification failed"]) {
        Some("authentication failed: the ssh host key is not trusted yet, connect once from a terminal (`ssh -T git@<host>`) to accept it")
    } else if has(&[
        "permission denied (publickey",
        "no matching host key",
        "no such identity",
    ]) {
        Some("authentication failed: no ssh key was accepted, check that `ssh-add -l` lists your key in a terminal")
    } else if has(&[
        "authentication failed",
        "could not read username",
        "could not read password",
        "terminal prompts disabled",
        "permission denied",
        "returned error: 401",
        "returned error: 403",
    ]) {
        Some("authentication failed: check your git credentials or ssh key")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(path: Option<&str>, sock: Option<&str>, ssh: Option<&str>) -> Vars {
        let own = |v: Option<&str>| v.map(str::to_string);
        Vars {
            path: own(path),
            ssh_auth_sock: own(sock),
            ssh_command: own(ssh),
        }
    }

    fn get<'a>(envs: &'a [(String, String)], key: &str) -> Option<&'a str> {
        envs.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    #[test]
    fn shell_output_is_read_through_noise() {
        let out = "welcome banner\n__townhall__PATH=/opt/homebrew/bin:/usr/bin\n__townhall__SSH_AUTH_SOCK=/tmp/agent.sock\n__townhall__GIT_SSH_COMMAND=\n";
        assert_eq!(
            parse_shell_output(out),
            vars(
                Some("/opt/homebrew/bin:/usr/bin"),
                Some("/tmp/agent.sock"),
                None
            )
        );
        assert_eq!(parse_shell_output("nothing useful"), Vars::default());
    }

    #[test]
    fn path_puts_the_shell_first_and_drops_repeats() {
        let got = merge_path(
            Some("/opt/homebrew/bin:/usr/bin"),
            Some("/usr/bin:/bin"),
            &["/usr/local/bin", "/bin"],
        );
        assert_eq!(got, "/opt/homebrew/bin:/usr/bin:/bin:/usr/local/bin");
    }

    #[test]
    fn path_still_has_homebrew_when_the_shell_gave_nothing() {
        let got = merge_path(None, Some("/usr/bin:/bin"), &FALLBACK_DIRS);
        assert_eq!(got, "/usr/bin:/bin:/opt/homebrew/bin:/usr/local/bin");
    }

    #[test]
    fn prompts_are_off_and_ssh_is_batch_by_default() {
        let envs = compose(&Vars::default(), &Vars::default(), None);
        assert_eq!(get(&envs, "GIT_TERMINAL_PROMPT"), Some("0"));
        assert_eq!(get(&envs, "GIT_SSH_COMMAND"), Some(SSH_NO_PROMPT));
        assert_eq!(get(&envs, "SSH_AUTH_SOCK"), None);
    }

    #[test]
    fn a_user_ssh_command_wins_over_the_shell_and_the_default() {
        let process = vars(None, None, Some("ssh -i /k"));
        let shell = vars(None, None, Some("ssh -i /other"));
        assert_eq!(
            get(&compose(&process, &shell, None), "GIT_SSH_COMMAND"),
            Some("ssh -i /k")
        );
        assert_eq!(
            get(&compose(&Vars::default(), &shell, None), "GIT_SSH_COMMAND"),
            Some("ssh -i /other")
        );
    }

    #[test]
    fn the_agent_socket_comes_from_process_then_shell_then_launchd() {
        let process = vars(None, Some("/p.sock"), None);
        let shell = vars(None, Some("/s.sock"), None);
        let sock = |p: &Vars, s: &Vars, l: Option<&str>| {
            get(&compose(p, s, l.map(str::to_string)), "SSH_AUTH_SOCK").map(str::to_string)
        };
        assert_eq!(
            sock(&process, &shell, Some("/l.sock")).as_deref(),
            Some("/p.sock")
        );
        assert_eq!(
            sock(&Vars::default(), &shell, Some("/l.sock")).as_deref(),
            Some("/s.sock")
        );
        assert_eq!(
            sock(&Vars::default(), &Vars::default(), Some("/l.sock")).as_deref(),
            Some("/l.sock")
        );
    }

    #[cfg(unix)]
    fn script(dir: &std::path::Path, body: &str) -> String {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join("fake-shell");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path.to_string_lossy().into_owned()
    }

    #[cfg(unix)]
    #[test]
    fn a_login_shell_is_asked_for_its_variables() {
        let dir = tempfile::tempdir().unwrap();
        let shell = script(
            dir.path(),
            "echo noise\necho __townhall__PATH=/brew/bin\necho __townhall__SSH_AUTH_SOCK=/a.sock",
        );
        let got = login_shell_vars(&shell, Duration::from_secs(5)).unwrap();
        assert_eq!(got, vars(Some("/brew/bin"), Some("/a.sock"), None));
    }

    #[cfg(unix)]
    #[test]
    fn a_shell_that_hangs_is_given_up_on() {
        let dir = tempfile::tempdir().unwrap();
        let shell = script(dir.path(), "sleep 30");
        let started = std::time::Instant::now();
        assert_eq!(login_shell_vars(&shell, Duration::from_millis(200)), None);
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    #[test]
    fn a_missing_shell_is_not_an_error() {
        assert_eq!(
            login_shell_vars("/no/such/shell", Duration::from_secs(1)),
            None
        );
    }

    #[test]
    fn failures_get_a_specific_hint() {
        let host = auth_failure(
            "Host key verification failed.\nfatal: Could not read from remote repository.",
        )
        .unwrap();
        assert!(host.contains("host key"), "{host}");
        let key = auth_failure("git@github.com: Permission denied (publickey).").unwrap();
        assert!(key.contains("ssh-add"), "{key}");
        let creds = auth_failure(
            "fatal: could not read Username for 'https://x': terminal prompts disabled",
        )
        .unwrap();
        assert!(creds.contains("credentials"), "{creds}");
        assert!(auth_failure("fatal: Could not resolve host: nowhere").is_none());
        for hint in [host, key, creds] {
            assert!(hint.starts_with("authentication failed"), "{hint}");
        }
    }
}
