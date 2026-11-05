#![allow(dead_code)]

//! A fake `claude` for townhall's own session, and helpers to drive conversations.
//! The fake logs its argv (NUL-separated), cwd, environment and stdin per turn and picks a script by a word in
//! the message: `SLOW` waits for the interrupt, `HANG` ignores it, `TOOL` asks for a
//! disallowed and an allowed tool, `CRASH` dies before it starts, `KIDS` leaves a
//! `sleep 31` holding its stdout (`ESCAPE` in its own process group) and ignores the
//! interrupt, `STALE` cannot find the session on `--resume`, `ALWAYS` never can,
//! `MENTION` succeeds with a reply (and a stderr line) that quotes that error, and
//! `ERRAFTER` ends with an error result quoting it after `init`.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use axum::http::StatusCode;
use axum::Router;
use serde_json::{json, Value};
use townhall_core::ContextOptions;
use townhall_daemon::{router, AppState};

use crate::common::{call, Env};

const SCRIPT: &str = r#"#!/bin/sh
dir='@DIR@'
n=$(( $(cat "$dir/count" 2>/dev/null || echo 0) + 1 ))
echo "$n" > "$dir/count"
printf '%s\0' "$@" > "$dir/argv-$n"
pwd -P > "$dir/cwd-$n"
echo "$$" > "$dir/pid-$n"
env > "$dir/env.tmp-$n" && mv "$dir/env.tmp-$n" "$dir/env-$n"
IFS= read -r first || exit 4
printf '%s\n' "$first" > "$dir/stdin-$n"
log() { printf '%s\n' "$1" >> "$dir/stdin-$n"; }
lost() { echo "No conversation found with session ID: $1" >&2; exit 1; }
case "$first" in
  *CRASH*) echo 'warming up' >&2; echo 'boom: claude broke' >&2; exit 3 ;;
  *ALWAYS*) lost any ;;
  *STALE*) case " $* " in *" --resume="*) lost resumed ;; esac ;;
esac
echo '{"type":"system","subtype":"init","session_id":"fake","tools":[]}'
case "$first" in
  *KIDS*|*ESCAPE*)
    case "$first" in
      *ESCAPE*)
        perl -e 'setpgrp(0,0); open F,">",shift; print F $$; close F; exec @ARGV' "$dir/kid-$n" sleep 31 &
        while [ ! -s "$dir/kid-$n" ]; do sleep 0.05; done ;;
      *) sleep 31 & echo "$!" > "$dir/kid-$n" ;;
    esac
    while IFS= read -r line; do log "$line"; done
    exit 0 ;;
  *SLOW*|*HANG*)
    while IFS= read -r line; do
      log "$line"
      case "$line" in
        *'"interrupt"'*)
          case "$first" in *HANG*) continue ;; esac
          echo '{"type":"control_response","response":{"subtype":"success","request_id":"x"}}'
          echo '{"type":"result","subtype":"error_during_execution","is_error":true,"terminal_reason":"aborted_streaming"}'
          cat >> "$dir/stdin-$n"
          exit 0 ;;
      esac
    done
    exit 0 ;;
  *ERRAFTER*)
    echo '{"type":"result","subtype":"error_during_execution","is_error":true,"result":"No conversation found with session ID: z"}'
    cat >> "$dir/stdin-$n"
    exit 1 ;;
  *TOOL*)
    echo '{"type":"control_request","request_id":"req-edit","request":{"subtype":"can_use_tool","tool_name":"Edit","input":{"file_path":"/x","old_string":"a","new_string":"b"},"tool_use_id":"toolu_1"}}'
    IFS= read -r answer; log "$answer"
    echo '{"type":"control_request","request_id":"req-read","request":{"subtype":"can_use_tool","tool_name":"Read","input":{"file_path":"/x"},"tool_use_id":"toolu_2"}}'
    IFS= read -r answer; log "$answer" ;;
esac
say='on it.'
case "$first" in
  *MENTION*) say='the app logs say: No conversation found with session ID 42'; echo "$say" >&2 ;;
esac
echo '{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"on"}}}'
echo '{"type":"assistant","message":{"model":"claude-test","role":"assistant","content":[{"type":"text","text":"'"$say"'"}],"usage":{"input_tokens":1200,"cache_creation_input_tokens":300,"cache_read_input_tokens":500,"output_tokens":20}}}'
echo '{"type":"result","subtype":"success","is_error":false,"result":"'"$say"'","usage":{"input_tokens":5000,"output_tokens":40},"modelUsage":{"claude-test":{"contextWindow":200000}}}'
cat >> "$dir/stdin-$n"
"#;

/// The fake's folder: its script and one log per turn.
pub struct Fake {
    pub dir: PathBuf,
}

impl Fake {
    pub fn install(parent: &Path) -> Self {
        let dir = parent.join("fake-claude");
        std::fs::create_dir_all(&dir).unwrap();
        let dir = dir.canonicalize().unwrap();
        let script = dir.join("claude");
        std::fs::write(&script, SCRIPT.replace("@DIR@", &dir.to_string_lossy())).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        Self { dir }
    }

    pub fn bin(&self) -> String {
        self.dir.join("claude").to_string_lossy().into_owned()
    }

    /// How many turns started.
    pub fn turns(&self) -> usize {
        std::fs::read_to_string(self.dir.join("count"))
            .map(|s| s.trim().parse().unwrap())
            .unwrap_or(0)
    }

    /// The turn's argv, one element each, newlines inside an element kept.
    pub fn argv(&self, turn: usize) -> Vec<String> {
        let text = std::fs::read_to_string(self.dir.join(format!("argv-{turn}"))).unwrap();
        text.split_terminator('\0').map(String::from).collect()
    }

    /// The pid of the fake's turn, or of the child it left behind (`kid`).
    pub fn pid(&self, what: &str, turn: usize) -> String {
        let file = self.dir.join(format!("{what}-{turn}"));
        for _ in 0..100 {
            match std::fs::read_to_string(&file) {
                Ok(text) if !text.trim().is_empty() => return text.trim().to_string(),
                _ => std::thread::sleep(std::time::Duration::from_millis(50)),
            }
        }
        panic!("{} was never written", file.display());
    }

    pub fn cwd(&self, turn: usize) -> PathBuf {
        let text = std::fs::read_to_string(self.dir.join(format!("cwd-{turn}"))).unwrap();
        PathBuf::from(text.trim())
    }

    /// The turn's environment, `NAME=value` lines, once the fake has written it.
    pub async fn env(&self, turn: usize) -> String {
        let file = self.dir.join(format!("env-{turn}"));
        for _ in 0..250 {
            if let Ok(text) = std::fs::read_to_string(&file) {
                return text;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("the fake never wrote the environment of turn {turn}");
    }

    /// Every stdin line of the turn, parsed.
    pub fn stdin(&self, turn: usize) -> Vec<Value> {
        let text = std::fs::read_to_string(self.dir.join(format!("stdin-{turn}"))).unwrap();
        text.lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }
}

/// A daemon over `env` whose claude is the fake.
pub fn app_with(env: &Env, fake: &Fake) -> (AppState, Router) {
    let options = ContextOptions {
        claude_bin: fake.bin(),
        ..ContextOptions::default()
    };
    let state = env.state_with(options);
    (state.clone(), router(state))
}

pub async fn create(app: &Router, text: &str) -> Value {
    let (s, v) = call(
        app,
        "POST",
        "/api/conversations",
        Some(json!({"text": text})),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
    v["data"].clone()
}

pub async fn message(app: &Router, id: &str, text: &str) -> (StatusCode, Value) {
    let uri = format!("/api/conversations/{id}/messages");
    call(app, "POST", &uri, Some(json!({"text": text}))).await
}

pub async fn detail(app: &Router, id: &str) -> Value {
    let (s, v) = call(app, "GET", &format!("/api/conversations/{id}"), None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    v["data"].clone()
}

/// How long a test waits for something that must happen. A deadline, not an attempt
/// count: a slow `detail` call under load must not shorten it.
const WAIT: Duration = Duration::from_secs(60);

/// Waits until the conversation is `yourTurn` again and returns its detail.
pub async fn idle(app: &Router, id: &str) -> Value {
    let deadline = tokio::time::Instant::now() + WAIT;
    while tokio::time::Instant::now() < deadline {
        let d = detail(app, id).await;
        if d["conversation"]["status"] == "yourTurn" {
            return d;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("conversation {id} is still thinking");
}

/// Waits until the fake has started `n` turns.
pub async fn turns_started(fake: &Fake, n: usize) {
    let deadline = tokio::time::Instant::now() + WAIT;
    while tokio::time::Instant::now() < deadline {
        if fake.turns() >= n && fake.dir.join(format!("stdin-{n}")).exists() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("the fake never started turn {n}");
}

pub fn kinds(detail: &Value) -> Vec<String> {
    detail["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["kind"].as_str().unwrap().to_string())
        .collect()
}

pub fn entries_of(detail: &Value, kind: &str) -> Vec<Value> {
    detail["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["kind"] == kind)
        .cloned()
        .collect()
}

/// The value of every `flag=value` element, in order. Every option is one element.
pub fn values_of(argv: &[String], flag: &str) -> Vec<String> {
    let prefix = format!("{flag}=");
    argv.iter()
        .filter_map(|a| a.strip_prefix(&prefix))
        .map(String::from)
        .collect()
}

pub async fn add_repo(app: &Router, path: &Path) {
    let (s, v) = call(app, "POST", "/api/repos", Some(json!({"path": path}))).await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
}

pub async fn set_setting(app: &Router, key: &str, value: Value) {
    let uri = format!("/api/settings/{key}");
    let (s, v) = call(app, "PUT", &uri, Some(json!({"value": value}))).await;
    assert_eq!(s, StatusCode::OK, "{v}");
}

pub fn alive(pid: &str) -> bool {
    std::process::Command::new("kill")
        .args(["-0", pid])
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Waits up to two seconds for `pid` to end.
pub async fn gone(pid: &str) -> bool {
    for _ in 0..100 {
        if !alive(pid) {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    false
}
