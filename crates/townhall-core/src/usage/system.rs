//! The real source: Claude Code's token from the macOS Keychain or its credentials file, and
//! the request through `curl`, since the daemon has no TLS client of its own. The token goes to
//! curl on stdin, never on its command line where `ps` would show it.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::timeout;

use super::{token_from_json, BoxFuture, HttpReply, UsageSource};
use crate::redact::redact;

const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const KEYCHAIN_SERVICE: &str = "Claude Code-credentials";
const KEYCHAIN_TIMEOUT: Duration = Duration::from_secs(3);
const REQUEST_SECS: u64 = 10;
const WAIT_FOR_CURL: Duration = Duration::from_secs(REQUEST_SECS + 5);

pub struct SystemSource {
    user: Option<String>,
    credentials: Option<PathBuf>,
}

impl SystemSource {
    pub fn from_env() -> Self {
        let var = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
        Self {
            user: var("USER").or_else(|| var("LOGNAME")),
            credentials: var("HOME").map(|h| PathBuf::from(h).join(".claude/.credentials.json")),
        }
    }

    /// The Keychain entry Claude Code writes; only macOS has `security`.
    async fn keychain_token(&self) -> Option<String> {
        if !cfg!(target_os = "macos") {
            return None;
        }
        let found = self.read_keychain(self.user.as_deref()?).await;
        found
            .inspect_err(|why| {
                tracing::debug!(
                    target: "townhall::usage", event = "usage.keychain_miss",
                    error = %redact(why), "no token in the Keychain"
                );
            })
            .ok()
    }

    async fn read_keychain(&self, user: &str) -> Result<String, String> {
        let mut command = Command::new("security");
        command
            .args([
                "find-generic-password",
                "-s",
                KEYCHAIN_SERVICE,
                "-a",
                user,
                "-w",
            ])
            .stdin(Stdio::null())
            .kill_on_drop(true);
        let output = timeout(KEYCHAIN_TIMEOUT, command.output())
            .await
            .map_err(|_| "security timed out".to_string())?
            .map_err(|e| format!("cannot run security: {e}"))?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
        }
        token_from_json(String::from_utf8_lossy(&output.stdout).trim())
            .ok_or_else(|| "the Keychain entry holds no access token".to_string())
    }
}

impl UsageSource for SystemSource {
    fn token(&self) -> BoxFuture<'_, Option<String>> {
        Box::pin(async move {
            if let Some(token) = self.keychain_token().await {
                return Some(token);
            }
            let path = self.credentials.clone()?;
            tokio::task::spawn_blocking(move || token_from_file(&path))
                .await
                .ok()
                .flatten()
        })
    }

    fn get<'a>(&'a self, token: &'a str) -> BoxFuture<'a, Result<HttpReply, String>> {
        Box::pin(async move {
            let config = curl_config(token).ok_or("the token is not a valid header value")?;
            let mut child = Command::new("curl")
                .args(["--silent", "--show-error", "--proto", "=https"])
                .args(["--max-time", &REQUEST_SECS.to_string(), "--config", "-"])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true)
                .spawn()
                .map_err(|e| format!("cannot run curl: {e}"))?;
            let mut stdin = child.stdin.take().ok_or("curl has no stdin")?;
            stdin
                .write_all(config.as_bytes())
                .await
                .map_err(|e| format!("cannot write curl's config: {e}"))?;
            drop(stdin);
            let output = timeout(WAIT_FOR_CURL, child.wait_with_output())
                .await
                .map_err(|_| "curl timed out".to_string())?
                .map_err(|e| format!("curl failed: {e}"))?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let code = output
                    .status
                    .code()
                    .map_or("a signal".to_string(), |c| c.to_string());
                return Err(format!("curl exited with {code}: {}", stderr.trim()));
            }
            parse_curl_output(&String::from_utf8_lossy(&output.stdout))
        })
    }
}

/// The request as a curl config read from stdin. A token with a control character could add
/// config lines of its own, so it is refused.
fn curl_config(token: &str) -> Option<String> {
    if token.is_empty() || token.chars().any(char::is_control) {
        return None;
    }
    let token = token.replace('\\', "\\\\").replace('"', "\\\"");
    Some(format!(
        "url = \"{USAGE_URL}\"\n\
         header = \"Authorization: Bearer {token}\"\n\
         header = \"anthropic-beta: oauth-2025-04-20\"\n\
         write-out = \"\\n%{{http_code}}\"\n"
    ))
}

/// The body, then the status curl's `write-out` put on the last line.
fn parse_curl_output(stdout: &str) -> Result<HttpReply, String> {
    let (body, code) = stdout
        .trim_end()
        .rsplit_once('\n')
        .ok_or("curl printed no status")?;
    let status = code
        .trim()
        .parse::<u16>()
        .map_err(|_| "curl printed no status".to_string())?;
    Ok(HttpReply {
        status,
        body: body.to_string(),
    })
}

fn token_from_file(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|raw| token_from_json(&raw))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_curl_config_carries_the_url_and_both_headers() {
        let config = curl_config("tok-1").unwrap();
        assert!(config.contains(r#"url = "https://api.anthropic.com/api/oauth/usage""#));
        assert!(config.contains(r#"header = "Authorization: Bearer tok-1""#));
        assert!(config.contains(r#"header = "anthropic-beta: oauth-2025-04-20""#));
        assert!(config.contains(r#"write-out = "\n%{http_code}""#));
    }

    #[test]
    fn a_token_cannot_break_out_of_its_config_line() {
        let config = curl_config(r#"a"b\c"#).unwrap();
        assert!(config.contains(r#"Bearer a\"b\\c""#), "{config}");
        for token in ["a\nurl = \"http://evil\"", "a\rb", "a\0b", ""] {
            assert_eq!(curl_config(token), None, "{token:?}");
        }
    }

    #[test]
    fn the_status_is_the_last_line_of_curls_output() {
        let reply = parse_curl_output("{\"a\":1}\n200").unwrap();
        assert_eq!(
            reply,
            HttpReply {
                status: 200,
                body: "{\"a\":1}".into()
            }
        );
        let multi = parse_curl_output("{\n\"a\": 1\n}\n429\n").unwrap();
        assert_eq!((multi.status, multi.body.as_str()), (429, "{\n\"a\": 1\n}"));
        let empty = parse_curl_output("\n503").unwrap();
        assert_eq!((empty.status, empty.body.as_str()), (503, ""));
    }

    #[test]
    fn output_without_a_status_is_an_error() {
        for out in ["", "{}", "{}\nnope", "{}\n99999"] {
            assert!(parse_curl_output(out).is_err(), "{out:?}");
        }
    }

    #[test]
    fn the_credentials_file_gives_the_token_or_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".credentials.json");
        assert_eq!(token_from_file(&path), None);
        std::fs::write(&path, r#"{"claudeAiOauth":{"accessToken":"tok-2"}}"#).unwrap();
        assert_eq!(token_from_file(&path), Some("tok-2".to_string()));
        std::fs::write(&path, "not json").unwrap();
        assert_eq!(token_from_file(&path), None);
    }
}
