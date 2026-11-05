//! The log reader: filters, bad lines, output caps and fingerprint grouping, on fixture
//! JSONL files in a temp folder.

use std::path::Path;

use serde_json::{json, Value};
use tempfile::TempDir;
use townhall_core::logs::{
    fingerprint, log_summary, parse_since, read_logs, ReadArgs, SummaryArgs, MAX_OUTPUT_BYTES,
};

/// 2026-11-05T12:00:00Z
const NOW: i64 = 1_793_880_000_000;

fn line(ts: &str, level: &str, target: &str, event: &str, message: &str, extra: Value) -> String {
    let mut v = json!({"timestamp": ts, "level": level, "target": target, "event": event, "message": message});
    for (k, val) in extra.as_object().unwrap() {
        v[k] = val.clone();
    }
    v.to_string()
}

fn write(dir: &Path, name: &str, lines: &[String]) {
    std::fs::write(dir.join(name), lines.join("\n") + "\n").unwrap();
}

/// A daemon file and an app file for today, an older daemon file, and junk.
fn fixture() -> TempDir {
    let dir = TempDir::new().unwrap();
    write(
        dir.path(),
        "daemon.2026-11-05.jsonl",
        &[
            line("2026-11-05T09:00:00.000000Z", "ERROR", "townhall::task", "task.launch_failed", "launch failed", json!({"task": "alpha", "error_kind": "command"})),
            "this is not json".into(),
            line("2026-11-05T10:00:00.000000Z", "WARN", "townhall::git", "cmd.run", "git worktree add failed", json!({"task": "beta"})),
            line("2026-11-05T10:30:00.000000Z", "INFO", "townhall::http", "http.request", "GET /api/tasks", json!({"status": 200})),
            "{\"no\": \"timestamp\"}".into(),
            line("2026-11-05T11:00:00.000000Z", "DEBUG", "townhall::http", "http.request", "debug noise", json!({})),
        ],
    );
    write(
        dir.path(),
        "app.2026-11-05.jsonl",
        &[line("2026-11-05T10:15:00.000000Z", "ERROR", "townhall::web", "web.error", "Boom in render", json!({"route": "/tasks"}))],
    );
    write(
        dir.path(),
        "daemon.2026-11-01.jsonl",
        &[line("2026-11-01T10:00:00.000000Z", "ERROR", "townhall::task", "task.launch_failed", "old failure", json!({"task": "ancient"}))],
    );
    std::fs::write(dir.path().join("notes.txt"), "ignored").unwrap();
    dir
}

fn read(dir: &Path, args: Value) -> Value {
    let args: ReadArgs = serde_json::from_value(args).unwrap();
    serde_json::to_value(read_logs(dir, NOW, &args).unwrap()).unwrap()
}

fn lines(v: &Value) -> Vec<String> {
    v["lines"].as_array().unwrap().iter().map(|l| l.as_str().unwrap().to_string()).collect()
}

#[test]
fn defaults_are_warn_and_up_newest_first_with_source() {
    let dir = fixture();
    let out = read(dir.path(), json!({}));
    let l = lines(&out);
    assert_eq!(l.len(), 3, "{l:?}");
    assert!(l[0].contains("10:15:00") && l[0].contains("web.error") && l[0].contains("app"), "{}", l[0]);
    assert!(l[1].contains("10:00:00") && l[1].contains("cmd.run") && l[1].contains("daemon"), "{}", l[1]);
    assert!(l[2].contains("09:00:00") && l[2].contains("task=alpha"), "{}", l[2]);
    assert_eq!(out["truncated"], false);
    assert!(!l.iter().any(|x| x.contains("old failure")), "older than 24h");
}

#[test]
fn level_is_a_minimum() {
    let dir = fixture();
    assert_eq!(lines(&read(dir.path(), json!({"level": "error"}))).len(), 2);
    assert_eq!(lines(&read(dir.path(), json!({"level": "info"}))).len(), 4);
    assert_eq!(lines(&read(dir.path(), json!({"level": "debug"}))).len(), 5);
}

#[test]
fn component_event_task_and_grep_filter() {
    let dir = fixture();
    let by = |args: Value| lines(&read(dir.path(), args));
    assert_eq!(by(json!({"component": "townhall::git"})).len(), 1);
    assert_eq!(by(json!({"component": "git"})).len(), 1, "short component names work");
    assert_eq!(by(json!({"component": "townhall::"})).len(), 3, "prefix");
    assert_eq!(by(json!({"event": "task.launch_failed"})).len(), 1);
    assert_eq!(by(json!({"task": "beta"})).len(), 1);
    assert_eq!(by(json!({"task": "nobody"})).len(), 0);
    assert_eq!(by(json!({"grep": "BOOM"})).len(), 1, "case-insensitive");
    assert_eq!(by(json!({"grep": "worktree", "level": "info"})).len(), 1);
}

#[test]
fn since_accepts_durations_and_rfc3339() {
    let dir = fixture();
    let by = |since: &str| lines(&read(dir.path(), json!({"since": since}))).len();
    assert_eq!(by("1h"), 0);
    assert_eq!(by("90m"), 0);
    // 10:15 and 10:00 are within 2h of 12:00, 09:00 is not.
    assert_eq!(by("2h"), 2);
    assert_eq!(by("3h"), 3);
    assert_eq!(by("2026-11-05T09:30:00Z"), 2);
}

#[test]
fn since_values() {
    assert_eq!(parse_since("24h", NOW).unwrap(), NOW - 86_400_000);
    assert_eq!(parse_since("7d", NOW).unwrap(), NOW - 7 * 86_400_000);
    assert_eq!(parse_since("30m", NOW).unwrap(), NOW - 1_800_000);
    assert_eq!(parse_since("2026-11-05T10:00:00Z", NOW).unwrap(), NOW - 7_200_000);
    assert_eq!(parse_since("2026-11-05T12:00:00.500Z", NOW).unwrap(), NOW + 500);
    assert_eq!(parse_since("2026-11-05T14:00:00+02:00", NOW).unwrap(), NOW);
    assert!(parse_since("yesterday", NOW).is_err());
    assert!(parse_since("", NOW).is_err());
    assert!(parse_since("0h", NOW).is_err());
    let all = read_logs(
        fixture().path(),
        NOW,
        &serde_json::from_value(json!({"since": "yesterday"})).unwrap(),
    );
    assert!(all.is_err());
}

#[test]
fn since_stops_at_rfc3339_boundary() {
    let dir = fixture();
    let out = read(dir.path(), json!({"since": "2026-11-05T10:05:00Z"}));
    assert_eq!(lines(&out).len(), 1);
    let out = read(dir.path(), json!({"since": "7d"}));
    assert_eq!(lines(&out).len(), 4, "the old file is read for 7d");
}

#[test]
fn limit_defaults_and_is_clamped() {
    let dir = TempDir::new().unwrap();
    let many: Vec<String> = (0..300)
        .map(|i| line(&format!("2026-11-05T10:{:02}:{:02}.000000Z", i / 60, i % 60), "ERROR", "townhall::x", "x.y", &format!("m{i}"), json!({})))
        .collect();
    write(dir.path(), "daemon.2026-11-05.jsonl", &many);
    assert_eq!(lines(&read(dir.path(), json!({}))).len(), 50);
    assert_eq!(lines(&read(dir.path(), json!({"limit": 500}))).len(), 200);
    let five = lines(&read(dir.path(), json!({"limit": 5})));
    assert_eq!(five.len(), 5);
    assert!(five[0].contains("m299"), "newest first: {}", five[0]);
}

#[test]
fn missing_dir_is_empty() {
    let dir = TempDir::new().unwrap();
    let none = dir.path().join("nope");
    let out = read(&none, json!({}));
    assert_eq!(lines(&out).len(), 0);
}

#[test]
fn each_line_is_cut_to_300_chars() {
    let dir = TempDir::new().unwrap();
    let long = "é".repeat(2000);
    write(
        dir.path(),
        "daemon.2026-11-05.jsonl",
        &[line("2026-11-05T10:00:00.000000Z", "ERROR", "townhall::x", "x.y", &long, json!({"task": "t"}))],
    );
    let l = lines(&read(dir.path(), json!({})));
    assert_eq!(l.len(), 1);
    assert!(l[0].chars().count() <= 300, "{}", l[0].chars().count());
}

#[test]
fn output_is_capped_at_16kb_with_truncated_flag() {
    let dir = TempDir::new().unwrap();
    let msg = "x".repeat(400);
    let many: Vec<String> = (0..200)
        .map(|i| line(&format!("2026-11-05T10:{:02}:{:02}.000000Z", i / 60, i % 60), "ERROR", "townhall::x", "x.y", &msg, json!({})))
        .collect();
    write(dir.path(), "daemon.2026-11-05.jsonl", &many);
    let out = read(dir.path(), json!({"limit": 200}));
    assert_eq!(out["truncated"], true);
    assert!(out.to_string().len() <= MAX_OUTPUT_BYTES, "{}", out.to_string().len());
    assert!(lines(&out).len() < 200);
    assert!(lines(&out).len() > 10);
}

#[test]
fn fingerprint_groups_same_message_with_different_numbers_and_names() {
    let a = fingerprint("townhall::task", "task.failed", "task foo-1 failed after 312ms", &[]);
    let b = fingerprint("townhall::task", "task.failed", "task bar-9 failed after 18ms", &[]);
    assert_eq!(a, b);
    assert_eq!(a.len(), 16);
    let c = fingerprint("townhall::task", "task.failed", "task foo-1 exploded after 312ms", &[]);
    assert_ne!(a, c);
    let d = fingerprint("townhall::git", "task.failed", "task foo-1 failed after 312ms", &[]);
    assert_ne!(a, d, "target is part of it");
    let e = fingerprint("townhall::task", "task.other", "task foo-1 failed after 312ms", &[]);
    assert_ne!(a, e, "event is part of it");
}

#[test]
fn fingerprint_normalizes_uuids_hex_paths_quotes_and_ids() {
    let f = |m: &str| fingerprint("t", "e", m, &[]);
    assert_eq!(
        f("open /Users/a/.townhall/x.db failed: 12 busy"),
        f("open /home/b/y/z.db failed: 7 busy")
    );
    assert_eq!(
        f("conversation 3f2b8c1e-1111-4222-8333-abcdefabcdef gone"),
        f("conversation 00000000-1111-4222-8333-123456789abc gone")
    );
    assert_eq!(f("commit deadbeef12 not found"), f("commit 0badc0de99 not found"));
    assert_eq!(f("no repo named 'shop'"), f("no repo named 'blog'"));
    assert_eq!(f("no repo named \"shop\""), f("no repo named \"blog\""));
    assert_eq!(f("can't find thing"), f("can't find thing"));
    assert_ne!(f("can't find thing"), f("can't lose thing"));
    // Values of id fields are placeholders too, even when they are plain words.
    let ids = [("task", "alpha")];
    assert_eq!(
        fingerprint("t", "e", "task alpha failed", &ids),
        fingerprint("t", "e", "task <name> failed", &[])
    );
}

fn summary(dir: &Path, args: Value) -> Value {
    let args: SummaryArgs = serde_json::from_value(args).unwrap();
    serde_json::to_value(log_summary(dir, NOW, &args).unwrap()).unwrap()
}

fn summary_fixture() -> TempDir {
    let dir = TempDir::new().unwrap();
    let t = |ts: &str, task: &str, msg: &str| line(ts, "ERROR", "townhall::task", "task.failed", msg, json!({"task": task}));
    write(
        dir.path(),
        "daemon.2026-11-05.jsonl",
        &[
            t("2026-11-05T08:00:00.000000Z", "foo-1", "task foo-1 failed after 312ms"),
            t("2026-11-05T09:00:00.000000Z", "bar-9", "task bar-9 failed after 18ms"),
            t("2026-11-05T10:00:00.000000Z", "baz-2", "task baz-2 failed after 7ms"),
            line("2026-11-05T10:30:00.000000Z", "WARN", "townhall::git", "cmd.run", "git failed token=abc123secret", json!({})),
            line("2026-11-05T11:00:00.000000Z", "ERROR", "townhall::panic", "panic", "index out of bounds", json!({"location": "src/a.rs:1:2"})),
            line("2026-11-05T11:30:00.000000Z", "INFO", "townhall::http", "http.request", "ignored below level", json!({})),
        ],
    );
    dir
}

#[test]
fn summary_counts_levels_components_and_panics() {
    let dir = summary_fixture();
    let s = summary(dir.path(), json!({}));
    assert_eq!(s["total"], 5);
    assert_eq!(s["by_level"]["ERROR"], 4);
    assert_eq!(s["by_level"]["WARN"], 1);
    assert!(s["by_level"].get("INFO").is_none());
    let comps = s["by_component"].as_array().unwrap();
    assert_eq!(comps[0], json!({"component": "townhall::task", "count": 3}));
    let panics = s["panics"].as_array().unwrap();
    assert_eq!(panics.len(), 1);
    assert_eq!(panics[0]["message"], "index out of bounds");
    assert_eq!(panics[0]["location"], "src/a.rs:1:2");
}

#[test]
fn summary_groups_recurring_failures() {
    let dir = summary_fixture();
    let s = summary(dir.path(), json!({}));
    let groups = s["groups"].as_array().unwrap();
    assert_eq!(groups.len(), 3, "{groups:?}");
    let top = &groups[0];
    assert_eq!(top["count"], 3);
    assert_eq!(top["first_seen"], "2026-11-05T08:00:00.000000Z");
    assert_eq!(top["last_seen"], "2026-11-05T10:00:00.000000Z");
    assert_eq!(top["tasks"], json!(["baz-2", "bar-9", "foo-1"]));
    assert_eq!(top["fingerprint"], fingerprint("townhall::task", "task.failed", "task foo-1 failed after 312ms", &[]));
    assert_eq!(top["target"], "townhall::task");
    assert_eq!(top["event"], "task.failed");
    let git = groups.iter().find(|g| g["target"] == "townhall::git").unwrap();
    let sample = git["sample"].as_str().unwrap();
    assert!(!sample.contains("abc123secret"), "{sample}");
}

#[test]
fn summary_top_limits_groups_and_tasks_are_capped_at_five() {
    let dir = TempDir::new().unwrap();
    let many: Vec<String> = (0..8)
        .map(|i| line(&format!("2026-11-05T10:00:0{i}.000000Z"), "ERROR", "townhall::task", "task.failed", "same", json!({"task": format!("t{i}")})))
        .chain((0..3).map(|i| line(&format!("2026-11-05T11:00:0{i}.000000Z"), "WARN", "townhall::git", "cmd.run", &format!("distinct kind {}", ["a", "b", "c"][i]), json!({}))))
        .collect();
    write(dir.path(), "daemon.2026-11-05.jsonl", &many);
    let s = summary(dir.path(), json!({"top": 2}));
    let groups = s["groups"].as_array().unwrap();
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0]["count"], 8);
    assert_eq!(groups[0]["tasks"].as_array().unwrap().len(), 5);
    let s = summary(dir.path(), json!({"top": 500}));
    assert!(s["groups"].as_array().unwrap().len() <= 50);
}

#[test]
fn summary_respects_since_and_level_and_the_size_cap() {
    let dir = summary_fixture();
    assert_eq!(summary(dir.path(), json!({"since": "30m"}))["total"], 0);
    assert_eq!(summary(dir.path(), json!({"level": "error"}))["total"], 4);

    let big = TempDir::new().unwrap();
    let many: Vec<String> = (0..60)
        .map(|i| line("2026-11-05T10:00:00.000000Z", "ERROR", &format!("townhall::c{i}"), &format!("e{i}"), &"w".repeat(400), json!({})))
        .collect();
    write(big.path(), "daemon.2026-11-05.jsonl", &many);
    let s = summary(big.path(), json!({"top": 50}));
    assert!(s.to_string().len() <= MAX_OUTPUT_BYTES, "{}", s.to_string().len());
    assert_eq!(s["truncated"], true);
}
