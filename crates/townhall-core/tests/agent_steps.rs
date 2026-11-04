use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

use tempfile::TempDir;
use townhall_core::steps::{
    latest_transcript_since, parse_transcript, project_dir_for, steps_for_agent, worktree_made,
};

const PAIRS: &str = include_str!("fixtures/transcripts/pairs.jsonl");
const MESSY: &str = include_str!("fixtures/transcripts/messy.jsonl");

fn ago(secs: u64) -> SystemTime {
    SystemTime::now() - Duration::from_secs(secs)
}

fn write_at(path: &Path, body: &str, at: SystemTime) {
    fs::write(path, body).unwrap();
    let file = fs::File::options().write(true).open(path).unwrap();
    file.set_modified(at).unwrap();
}

#[test]
fn latest_transcript_since_ignores_transcripts_written_before_the_cutoff() {
    let projects = TempDir::new().unwrap();
    let cwd = Path::new("/work/reused");
    let dir = project_dir_for(projects.path(), cwd);
    fs::create_dir_all(&dir).unwrap();
    write_at(&dir.join("old.jsonl"), "", ago(300));
    let since = ago(60);
    assert_eq!(
        latest_transcript_since(projects.path(), cwd, since).unwrap(),
        None
    );
    write_at(&dir.join("new.jsonl"), "", ago(10));
    assert_eq!(
        latest_transcript_since(projects.path(), cwd, since).unwrap(),
        Some(dir.join("new.jsonl"))
    );
    write_at(&dir.join("exact.jsonl"), "", since);
    write_at(&dir.join("new.jsonl"), "", ago(120));
    assert_eq!(
        latest_transcript_since(projects.path(), cwd, since).unwrap(),
        Some(dir.join("exact.jsonl")),
        "a transcript written at the cutoff counts"
    );
}

#[test]
fn a_worktree_was_made_when_its_folder_was_created() {
    let root = TempDir::new().unwrap();
    let before = SystemTime::now() - Duration::from_secs(1);
    let worktree = root.path().join("agent");
    fs::create_dir(&worktree).unwrap();
    fs::write(worktree.join(".git"), "gitdir: /x\n").unwrap();
    let made = worktree_made(&worktree).unwrap().unwrap();
    assert!(made >= before, "{made:?} before {before:?}");
    assert!(made <= SystemTime::now());
    fs::write(worktree.join("new-file.txt"), "x").unwrap();
    assert_eq!(
        worktree_made(&worktree).unwrap(),
        Some(made),
        "adding files does not move it"
    );
    assert_eq!(worktree_made(&root.path().join("gone")).unwrap(), None);
}

#[tokio::test]
async fn steps_for_agent_skips_a_transcript_older_than_the_worktree() {
    let projects = TempDir::new().unwrap();
    let worktree = TempDir::new().unwrap();
    let dir = project_dir_for(projects.path(), worktree.path());
    fs::create_dir_all(&dir).unwrap();
    write_at(&dir.join("earlier-agent.jsonl"), MESSY, ago(3_600));
    let steps = steps_for_agent(projects.path(), worktree.path())
        .await
        .unwrap();
    assert!(steps.is_empty(), "{steps:?}");

    write_at(&dir.join("this-agent.jsonl"), PAIRS, SystemTime::now());
    let steps = steps_for_agent(projects.path(), worktree.path())
        .await
        .unwrap();
    assert_eq!(steps, parse_transcript(PAIRS));
}

#[tokio::test]
async fn steps_for_agent_is_empty_without_a_worktree_or_transcript() {
    let projects = TempDir::new().unwrap();
    let root = TempDir::new().unwrap();
    let gone = root.path().join("gone");
    let dir = project_dir_for(projects.path(), &gone);
    fs::create_dir_all(&dir).unwrap();
    write_at(&dir.join("a.jsonl"), PAIRS, SystemTime::now());
    assert!(steps_for_agent(projects.path(), &gone)
        .await
        .unwrap()
        .is_empty());
    assert!(steps_for_agent(projects.path(), root.path())
        .await
        .unwrap()
        .is_empty());
}
