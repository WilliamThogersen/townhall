use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use tempfile::TempDir;
use townhall_core::steps::{
    latest_transcript, parse_transcript, project_dir_for, steps_for_worktree, Step, StepResult,
};

const PAIRS: &str = include_str!("fixtures/transcripts/pairs.jsonl");
const MESSY: &str = include_str!("fixtures/transcripts/messy.jsonl");

fn step(id: &str, tool: &str, summary: &str, started: &str, ended: Option<&str>) -> Step {
    Step {
        id: id.to_string(),
        tool: tool.to_string(),
        summary: summary.to_string(),
        started_at: started.to_string(),
        ended_at: ended.map(str::to_string),
        result: if ended.is_some() {
            StepResult::Ok
        } else {
            StepResult::Running
        },
    }
}

fn failed(s: Step) -> Step {
    Step {
        result: StepResult::Error,
        ..s
    }
}

fn at(second: u32) -> String {
    format!("2026-03-10T09:00:{second:02}.000Z")
}

fn write_aged(path: &Path, body: &str, age_secs: u64) {
    fs::write(path, body).unwrap();
    let file = fs::File::options().write(true).open(path).unwrap();
    file.set_modified(SystemTime::now() - Duration::from_secs(age_secs))
        .unwrap();
}

fn project(projects: &Path, cwd: &Path) -> PathBuf {
    let dir = project_dir_for(projects, cwd);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn tool_uses_pair_with_their_results() {
    let expected = vec![
        step("toolu_read", "Read", "src/login.rs", &at(2), Some(&at(3))),
        step(
            "toolu_edit",
            "Edit",
            "/elsewhere/notes.md",
            &at(4),
            Some(&at(9)),
        ),
        failed(step(
            "toolu_bash",
            "Bash",
            "cargo test -p townhall-core",
            &at(4),
            Some(&at(9)),
        )),
        step("toolu_grep", "Grep", "fn login", &at(10), Some(&at(11))),
        step("toolu_agent", "Agent", "review the fix", &at(12), None),
    ];
    assert_eq!(parse_transcript(PAIRS), expected);
}

#[test]
fn bad_lines_are_skipped_and_steps_ordered_by_start() {
    let expected = vec![
        failed(step(
            "toolu_early",
            "Task",
            "explore the repo",
            "2026-03-10T10:00:01.000Z",
            Some("2026-03-10T10:00:08.000Z"),
        )),
        Step {
            ended_at: None,
            result: StepResult::Ok,
            ..step(
                "toolu_late",
                "Glob",
                "**/*.rs",
                "2026-03-10T10:00:05.000Z",
                None,
            )
        },
    ];
    assert_eq!(parse_transcript(MESSY), expected);
}

#[test]
fn empty_or_garbage_input_gives_no_steps() {
    assert!(parse_transcript("").is_empty());
    assert!(parse_transcript("\n\n  \n").is_empty());
    assert!(parse_transcript("{\"type\":\"assistant\"").is_empty());
    assert!(parse_transcript("null\n42\n{}\n").is_empty());
}

#[test]
fn steps_serialize_in_camel_case() {
    let steps = parse_transcript(PAIRS);
    let json = serde_json::to_value(&steps[2..]).unwrap();
    assert_eq!(json[0]["startedAt"], "2026-03-10T09:00:04.000Z");
    assert_eq!(json[0]["endedAt"], "2026-03-10T09:00:09.000Z");
    assert_eq!(json[0]["result"], "error");
    assert_eq!(json[1]["result"], "ok");
    assert_eq!(json[2]["result"], "running");
    assert!(json[2]["endedAt"].is_null());
    let back: Vec<Step> = serde_json::from_value(json).unwrap();
    assert_eq!(back, steps[2..].to_vec());
}

#[test]
fn project_dir_replaces_every_non_alphanumeric() {
    let projects = Path::new("/home/me/.claude/projects");
    let cwd = Path::new("/home/me/.townhall/worktrees/shop-1a2b/fix_login");
    assert_eq!(
        project_dir_for(projects, cwd),
        projects.join("-home-me--townhall-worktrees-shop-1a2b-fix-login")
    );
}

#[test]
fn project_dir_counts_utf16_units_like_claude() {
    let projects = Path::new("/p");
    assert_eq!(
        project_dir_for(projects, Path::new("/tmp/café")),
        projects.join("-tmp-caf-")
    );
    assert_eq!(
        project_dir_for(projects, Path::new("/tmp/a\u{1F600}")),
        projects.join("-tmp-a--")
    );
}

/// Expected suffixes come from Claude Code 2.1.318's own `sanitizePath`, run in node.
#[test]
fn project_dir_over_200_chars_keeps_200_and_adds_claudes_hash() {
    let projects = Path::new("/p");
    let cwd = format!("/work/{}x", "segment_".repeat(27));
    assert_eq!(cwd.len(), 223);
    let sanitized = cwd.replace(|c: char| !c.is_ascii_alphanumeric(), "-");
    assert_eq!(
        project_dir_for(projects, Path::new(&cwd)),
        projects.join(format!("{}-s2ol9h", &sanitized[..200]))
    );
}

#[test]
fn project_dir_hash_runs_over_utf16_units() {
    let projects = Path::new("/p");
    let cwd = format!(
        "/work/café/{}/{}",
        "\u{1F600}".repeat(3),
        "segment_".repeat(26)
    );
    let sanitized = format!("-work-caf---------{}", "segment-".repeat(26));
    assert_eq!(
        project_dir_for(projects, Path::new(&cwd)),
        projects.join(format!("{}-3lm8in", &sanitized[..200]))
    );
}

#[test]
fn project_dir_is_hashed_only_past_200_chars() {
    let projects = Path::new("/p");
    let fits = format!("/{}", "a".repeat(199));
    assert_eq!(
        project_dir_for(projects, Path::new(&fits)),
        projects.join(format!("-{}", "a".repeat(199)))
    );
    let over = format!("/{}", "a".repeat(200));
    assert_eq!(
        project_dir_for(projects, Path::new(&over)),
        projects.join(format!("-{}-b6ymvl", "a".repeat(199)))
    );
}

#[test]
fn latest_transcript_picks_the_newest_jsonl() {
    let projects = TempDir::new().unwrap();
    let cwd = Path::new("/work/repo-that-does-not-exist");
    let dir = project(projects.path(), cwd);
    write_aged(&dir.join("old.jsonl"), "", 300);
    write_aged(&dir.join("new.jsonl"), "", 30);
    write_aged(&dir.join("notes.txt"), "", 0);
    fs::create_dir(dir.join("subdir.jsonl")).unwrap();
    let found = latest_transcript(projects.path(), cwd).unwrap();
    assert_eq!(found, Some(dir.join("new.jsonl")));
}

#[test]
fn latest_transcript_is_none_without_a_folder_or_files() {
    let projects = TempDir::new().unwrap();
    let cwd = Path::new("/work/empty");
    assert_eq!(latest_transcript(projects.path(), cwd).unwrap(), None);
    let dir = project(projects.path(), cwd);
    write_aged(&dir.join("readme.md"), "", 0);
    assert_eq!(latest_transcript(projects.path(), cwd).unwrap(), None);
}

#[test]
fn latest_transcript_errors_when_the_folder_is_unreadable() {
    let projects = TempDir::new().unwrap();
    let cwd = Path::new("/work/broken");
    fs::write(project_dir_for(projects.path(), cwd), "not a folder").unwrap();
    let err = latest_transcript(projects.path(), cwd).unwrap_err();
    assert!(err.to_string().contains("-work-broken"), "{err}");
}

#[cfg(unix)]
#[test]
fn latest_transcript_looks_under_the_resolved_path_too() {
    let projects = TempDir::new().unwrap();
    let root = TempDir::new().unwrap();
    let real = root.path().join("real");
    fs::create_dir(&real).unwrap();
    let link = root.path().join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let resolved = project(projects.path(), &real.canonicalize().unwrap());
    write_aged(&resolved.join("a.jsonl"), "", 60);
    assert_eq!(
        latest_transcript(projects.path(), &link).unwrap(),
        Some(resolved.join("a.jsonl"))
    );
    let given = project(projects.path(), &link);
    write_aged(&given.join("b.jsonl"), "", 5);
    assert_eq!(
        latest_transcript(projects.path(), &link).unwrap(),
        Some(given.join("b.jsonl"))
    );
}

#[tokio::test]
async fn steps_for_worktree_reads_the_newest_transcript() {
    let projects = TempDir::new().unwrap();
    let worktree = TempDir::new().unwrap();
    let dir = project(projects.path(), worktree.path());
    write_aged(&dir.join("older.jsonl"), MESSY, 120);
    write_aged(&dir.join("newer.jsonl"), PAIRS, 10);
    let steps = steps_for_worktree(projects.path(), worktree.path())
        .await
        .unwrap();
    assert_eq!(steps, parse_transcript(PAIRS));
}

#[tokio::test]
async fn steps_for_worktree_is_empty_without_a_transcript() {
    let projects = TempDir::new().unwrap();
    let worktree = TempDir::new().unwrap();
    let steps = steps_for_worktree(projects.path(), worktree.path())
        .await
        .unwrap();
    assert!(steps.is_empty());
}
