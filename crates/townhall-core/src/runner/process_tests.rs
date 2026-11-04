use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::sync::watch;

use super::*;

const LIMIT: Duration = Duration::from_secs(10);

fn sh(script: &str) -> Proc {
    let args = vec!["-c".to_string(), script.to_string()];
    Proc::spawn("/bin/sh", &args, Path::new("/")).unwrap()
}

fn alive(pid: &str) -> bool {
    std::process::Command::new("kill")
        .args(["-0", pid])
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

async fn gone(pid: &str) -> bool {
    for _ in 0..100 {
        if !alive(pid) {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    false
}

/// A script that starts `sleep 31` holding its stdout, says `ready`, then ignores stdin.
fn with_kid(dir: &Path, own_group: bool) -> (Proc, PathBuf) {
    let kid = dir.join("kid");
    let monitor = if own_group { "set -m; " } else { "" };
    let script = format!(
        "{monitor}sleep 31 & echo $! > '{}'; echo ready; while read l; do :; done",
        kid.display()
    );
    (sh(&script), kid)
}

#[tokio::test]
async fn reads_lines_and_the_stderr_tail() {
    let mut p = sh("read l; echo \"got $l\"; echo one >&2; echo last >&2; exit 2");
    let (_tx, mut stop) = watch::channel(false);
    p.write("hi\n").await;
    assert_eq!(p.next_line(&mut stop).await.as_deref(), Some("got hi"));
    assert_eq!(p.next_line(&mut stop).await, None);
    let exit = p.finish().await;
    assert_eq!(exit.code, Some(2));
    assert_eq!(exit.last_error.as_deref(), Some("last"));
    assert!(!exit.stopped);
}

#[tokio::test]
async fn a_stop_interrupts_on_stdin() {
    let mut p = sh("read l; echo ready; read l; echo \"$l\"");
    let (tx, mut stop) = watch::channel(false);
    p.write("go\n").await;
    assert_eq!(p.next_line(&mut stop).await.as_deref(), Some("ready"));
    tx.send_replace(true);
    let line = p.next_line(&mut stop).await.unwrap();
    assert!(line.contains("\"interrupt\""), "{line}");
    assert_eq!(p.next_line(&mut stop).await, None);
    assert!(p.finish().await.stopped);
}

#[tokio::test]
async fn a_bad_line_is_skipped_and_the_rest_is_read() {
    let mut p = sh("printf 'one\\n\\377\\376 bad\\r\\n{\"two\":2}'");
    let (_tx, mut stop) = watch::channel(false);
    assert_eq!(p.next_line(&mut stop).await.as_deref(), Some("one"));
    let bad = p.next_line(&mut stop).await.unwrap();
    assert!(bad.ends_with(" bad") && bad.contains('\u{fffd}'), "{bad:?}");
    assert_eq!(p.next_line(&mut stop).await.as_deref(), Some("{\"two\":2}"));
    assert_eq!(p.next_line(&mut stop).await, None);
    assert_eq!(p.finish().await.code, Some(0));
}

#[tokio::test]
async fn a_kill_ends_the_whole_process_group() {
    let dir = tempfile::TempDir::new().unwrap();
    let (mut p, kid) = with_kid(dir.path(), false);
    let (_tx, mut stop) = watch::channel(false);
    assert_eq!(p.next_line(&mut stop).await.as_deref(), Some("ready"));
    p.kill();
    let end = tokio::time::timeout(LIMIT, p.next_line(&mut stop)).await;
    assert_eq!(end.expect("stdout ends once the group is killed"), None);
    let exit = tokio::time::timeout(LIMIT, p.finish()).await.unwrap();
    assert_eq!(exit.code, None, "killed by a signal");
    let pid = std::fs::read_to_string(kid).unwrap();
    assert!(gone(pid.trim()).await, "the grandchild was killed too");
}

#[tokio::test]
async fn an_escaped_grandchild_does_not_hold_the_turn() {
    let dir = tempfile::TempDir::new().unwrap();
    let (mut p, kid) = with_kid(dir.path(), true);
    let (_tx, mut stop) = watch::channel(false);
    assert_eq!(p.next_line(&mut stop).await.as_deref(), Some("ready"));
    let pid = std::fs::read_to_string(kid).unwrap().trim().to_string();
    let started = tokio::time::Instant::now();
    p.kill();
    let end = tokio::time::timeout(LIMIT, p.next_line(&mut stop)).await;
    assert_eq!(end.expect("reading gives up after the kill"), None);
    tokio::time::timeout(LIMIT, p.finish()).await.unwrap();
    assert!(
        started.elapsed() < Duration::from_secs(6),
        "{:?}",
        started.elapsed()
    );
    assert!(alive(&pid), "it left the group, so it outlived the kill");
    std::process::Command::new("kill")
        .args(["-9", &pid])
        .status()
        .unwrap();
}

#[tokio::test]
async fn dropping_a_running_process_kills_its_group() {
    let dir = tempfile::TempDir::new().unwrap();
    let (mut p, kid) = with_kid(dir.path(), false);
    let (_tx, mut stop) = watch::channel(false);
    assert_eq!(p.next_line(&mut stop).await.as_deref(), Some("ready"));
    drop(p);
    let pid = std::fs::read_to_string(kid).unwrap();
    assert!(gone(pid.trim()).await);
}

#[test]
fn a_missing_program_is_a_spawn_error() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let err = rt
        .block_on(async { Proc::spawn("/no/claude", &[], Path::new("/")) })
        .unwrap_err();
    assert!(matches!(err, Error::Spawn { .. }), "{err}");
    assert_eq!(last_line("a\n b \n\n"), Some("b".into()));
    assert_eq!(last_line(" \n"), None);
}
