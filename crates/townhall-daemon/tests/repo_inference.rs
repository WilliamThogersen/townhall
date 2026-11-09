use std::path::Path;

use axum::http::StatusCode;
use serde_json::{json, Value};
use townhall_core::{ContextOptions, Workspace};
use townhall_daemon::router;

mod common;
mod runner_support;
use common::{call, git, Env};
use runner_support::{
    add_repo, app_with, create, entries_of, idle, kinds, message, set_setting, values_of, Fake,
};

async fn commit_file(repo: &Path, file: &str, body: &str) {
    let full = repo.join(file);
    std::fs::create_dir_all(full.parent().unwrap()).unwrap();
    std::fs::write(&full, body).unwrap();
    git(repo, &["add", "-A"]).await;
    git(
        repo,
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-q",
            "-m",
            "f",
        ],
    )
    .await;
}

fn repo_entry(repo: &str, rule: &str, reason: &str) -> Value {
    json!({"kind": "repo", "repo": repo, "baseBranch": "main", "rule": rule, "reason": reason})
}

fn bodies(entries: Vec<Value>) -> Vec<Value> {
    entries
        .into_iter()
        .map(|mut e| {
            let o = e.as_object_mut().unwrap();
            o.remove("id");
            o.remove("at");
            e
        })
        .collect()
}

fn id_of(created: &Value) -> String {
    created["conversation"]["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn the_only_repo_is_used_once_and_not_repeated() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (_state, app) = app_with(&env, &fake);
    let id = id_of(&create(&app, "hello").await);
    idle(&app, &id).await;
    message(&app, &id, "more").await;
    let d = idle(&app, &id).await;
    assert_eq!(
        bodies(entries_of(&d, "repo")),
        [repo_entry("alpha", "only", "only repo")]
    );
}

#[tokio::test]
async fn a_named_repo_wins_and_runs_the_session_there() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (_state, app) = app_with(&env, &fake);
    let beta = env.repo("beta").await;
    add_repo(&app, &beta).await;
    let id = id_of(&create(&app, "the beta login is broken").await);
    let d = idle(&app, &id).await;
    assert_eq!(
        bodies(entries_of(&d, "repo")),
        [repo_entry("beta", "named", "you named it")]
    );
    assert_eq!(fake.cwd(1), beta);
    assert_eq!(
        values_of(&fake.argv(1), "--add-dir"),
        [env.startup.to_string_lossy()]
    );
    let prompt = values_of(&fake.argv(1), "--append-system-prompt").join(" ");
    assert!(prompt.contains("repo for this turn: beta"), "{prompt}");

    message(&app, &id, "now alpha too").await;
    let d = idle(&app, &id).await;
    let repos = bodies(entries_of(&d, "repo"));
    assert_eq!(repos[1], repo_entry("alpha", "named", "you named it"));
    assert_eq!(fake.cwd(2), beta, "a resumed session keeps its folder");
}

#[tokio::test]
async fn the_code_rule_finds_the_repo_with_the_file() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (_state, app) = app_with(&env, &fake);
    let shop = env.repo("shop").await;
    commit_file(&shop, "src/search/query.ts", "export const q = 1\n").await;
    add_repo(&app, &shop).await;
    let id = id_of(&create(&app, "query.ts is slow").await);
    let d = idle(&app, &id).await;
    assert_eq!(
        bodies(entries_of(&d, "repo")),
        [repo_entry("shop", "code", "it has src/search/query.ts")]
    );
}

#[tokio::test]
async fn naming_two_repos_asks_and_the_answer_runs_the_first_message() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (_state, app) = app_with(&env, &fake);
    let beta = env.repo("beta").await;
    add_repo(&app, &beta).await;
    let id = id_of(&create(&app, "make it faster in alpha or beta").await);
    let d = idle(&app, &id).await;
    assert_eq!(kinds(&d), ["you", "repoAsk"]);
    assert_eq!(
        bodies(entries_of(&d, "repoAsk")),
        [
            json!({"kind": "repoAsk", "question": "you named two repos. which one?", "candidates": ["alpha", "beta"], "offerAll": true})
        ]
    );
    assert_eq!(fake.turns(), 0, "no turn until you answer");

    let (s, _) = message(&app, &id, "beta").await;
    assert_eq!(s, StatusCode::OK);
    let d = idle(&app, &id).await;
    assert_eq!(kinds(&d), ["you", "repoAsk", "you", "repo", "townhall"]);
    assert_eq!(
        bodies(entries_of(&d, "repo")),
        [repo_entry("beta", "picked", "you picked it")]
    );
    let text = &fake.stdin(1)[0]["message"]["content"][0]["text"];
    assert_eq!(text, "make it faster in alpha or beta");
    assert_eq!(fake.cwd(1), beta);

    message(&app, &id, "and the docs").await;
    let d = idle(&app, &id).await;
    assert_eq!(
        entries_of(&d, "repoAsk").len(),
        1,
        "it keeps the picked repo"
    );
    assert_eq!(fake.turns(), 2);
}

#[tokio::test]
async fn both_runs_the_turn_with_every_candidate() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (_state, app) = app_with(&env, &fake);
    add_repo(&app, &env.repo("beta").await).await;
    let id = id_of(&create(&app, "bump the version in alpha and beta").await);
    idle(&app, &id).await;
    message(&app, &id, "both").await;
    let d = idle(&app, &id).await;
    assert_eq!(
        bodies(entries_of(&d, "repo")),
        [
            repo_entry("alpha", "picked", "you picked both"),
            repo_entry("beta", "picked", "you picked both")
        ]
    );
    let prompt = values_of(&fake.argv(1), "--append-system-prompt").join(" ");
    assert!(prompt.contains("alpha, beta"), "{prompt}");
}

#[tokio::test]
async fn guess_picks_the_first_candidate_and_says_why() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (_state, app) = app_with(&env, &fake);
    add_repo(&app, &env.repo("beta").await).await;
    set_setting(&app, "repoUnclear", json!("guess")).await;
    let id = id_of(&create(&app, "make it faster in alpha or beta").await);
    let d = idle(&app, &id).await;
    assert_eq!(
        bodies(entries_of(&d, "repo")),
        [repo_entry(
            "alpha",
            "picked",
            "guessed: you named two repos"
        )]
    );
    assert_eq!(fake.turns(), 1);
}

#[tokio::test]
async fn plain_talk_with_several_repos_runs_without_a_repo() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (_state, app) = app_with(&env, &fake);
    add_repo(&app, &env.repo("beta").await).await;
    let id = id_of(&create(&app, "looking into the bend language").await);
    let d = idle(&app, &id).await;
    assert_eq!(kinds(&d), ["you", "townhall"]);
    assert_eq!(fake.turns(), 1);
    let prompt = values_of(&fake.argv(1), "--append-system-prompt").join(" ");
    assert!(
        prompt.contains("no repo is resolved for this turn"),
        "{prompt}"
    );
}

#[tokio::test]
async fn guess_does_not_pick_a_repo_for_plain_talk() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (_state, app) = app_with(&env, &fake);
    add_repo(&app, &env.repo("beta").await).await;
    set_setting(&app, "repoUnclear", json!("guess")).await;
    let id = id_of(&create(&app, "looking into the bend language").await);
    let d = idle(&app, &id).await;
    assert_eq!(kinds(&d), ["you", "townhall"]);
}

#[tokio::test]
async fn a_conversation_repo_is_kept_on_a_message_with_no_signal() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (_state, app) = app_with(&env, &fake);
    let beta = env.repo("beta").await;
    add_repo(&app, &beta).await;
    let id = id_of(&create(&app, "the beta login is broken").await);
    idle(&app, &id).await;
    message(&app, &id, "and why is that").await;
    let d = idle(&app, &id).await;
    assert_eq!(
        bodies(entries_of(&d, "repo")),
        [repo_entry("beta", "named", "you named it")]
    );
    assert!(entries_of(&d, "repoAsk").is_empty());
    let prompt = values_of(&fake.argv(2), "--append-system-prompt").join(" ");
    assert!(prompt.contains("repo for this turn: beta"), "{prompt}");
}

#[tokio::test]
async fn a_linked_repo_wins_once_per_change() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (_state, app) = app_with(&env, &fake);
    add_repo(&app, &env.repo("beta").await).await;
    let id = id_of(&create(&app, "alpha needs a page").await);
    idle(&app, &id).await;
    let uri = format!("/api/conversations/{id}");
    call(&app, "PATCH", &uri, Some(json!({"linkedRepo": "beta"}))).await;
    message(&app, &id, "alpha again").await;
    idle(&app, &id).await;
    message(&app, &id, "and more").await;
    let d = idle(&app, &id).await;
    assert_eq!(
        bodies(entries_of(&d, "repo")),
        [
            repo_entry("alpha", "named", "you named it"),
            repo_entry("beta", "linked", "you linked it")
        ]
    );
    let prompt = values_of(&fake.argv(3), "--append-system-prompt").join(" ");
    assert!(prompt.contains("repo for this turn: beta"), "{prompt}");
}

#[tokio::test]
async fn with_no_repos_the_turn_still_runs_in_the_home() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let plain = env.root.path().join("plain");
    std::fs::create_dir_all(&plain).unwrap();
    let options = ContextOptions {
        claude_bin: fake.bin(),
        ..ContextOptions::default()
    };
    let workspace = Workspace::new(env.home.clone(), &plain).unwrap();
    let app = router(townhall_daemon::AppState::with_options(workspace, options));
    let id = id_of(&create(&app, "hello").await);
    let d = idle(&app, &id).await;
    assert_eq!(kinds(&d), ["you", "townhall"]);
    assert_eq!(fake.cwd(1), env.home.root().canonicalize().unwrap());
    assert!(values_of(&fake.argv(1), "--add-dir").is_empty());
    let prompt = values_of(&fake.argv(1), "--append-system-prompt").join(" ");
    assert!(prompt.contains("no repo is registered"), "{prompt}");
}
