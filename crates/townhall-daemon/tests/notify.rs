use std::time::Duration;

use townhall_core::runner::notify;
use townhall_core::ContextOptions;
use townhall_daemon::router;

mod common;
mod runner_support;
use common::{call, Env};
use runner_support::{create, detail, entries_of, idle, kinds, turns_started, Fake};

fn quick(fake: &Fake) -> ContextOptions {
    ContextOptions {
        claude_bin: fake.bin(),
        notify_delay: Duration::from_millis(100),
        ..ContextOptions::default()
    }
}

fn id_of(created: &serde_json::Value) -> String {
    created["conversation"]["id"].as_str().unwrap().to_string()
}

async fn settle() {
    tokio::time::sleep(Duration::from_millis(600)).await;
}

#[tokio::test]
async fn a_task_event_wakes_the_planner_and_its_reply_wakes_nothing() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let state = env.state_with(quick(&fake));
    let app = router(state.clone());
    let id = id_of(&create(&app, "hello").await);
    idle(&app, &id).await;
    notify(state.ctx(), &id, "task fix ended: it is done".into());
    turns_started(&fake, 2).await;
    let d = idle(&app, &id).await;
    let events = entries_of(&d, "event");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["text"], "task fix ended: it is done");
    let k = kinds(&d);
    assert_eq!(k[k.len() - 2..], ["event", "townhall"], "{k:?}");
    let sent = fake.stdin(2)[0].to_string();
    assert!(sent.contains("task fix ended: it is done"), "{sent}");
    settle().await;
    assert_eq!(fake.turns(), 2, "the reply is no event");
    let _ = call;
}

#[tokio::test]
async fn events_close_together_make_one_turn() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let state = env.state_with(quick(&fake));
    let app = router(state.clone());
    let id = id_of(&create(&app, "hello").await);
    idle(&app, &id).await;
    notify(state.ctx(), &id, "task a ended".into());
    notify(state.ctx(), &id, "task b failed".into());
    turns_started(&fake, 2).await;
    let d = idle(&app, &id).await;
    settle().await;
    assert_eq!(fake.turns(), 2);
    assert_eq!(entries_of(&d, "event").len(), 2);
    let sent = fake.stdin(2)[0].to_string();
    assert!(sent.contains("task a ended") && sent.contains("task b failed"));
}

#[tokio::test]
async fn an_event_waits_for_the_running_turn() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let state = env.state_with(quick(&fake));
    let app = router(state.clone());
    let id = id_of(&create(&app, "SLOW").await);
    turns_started(&fake, 1).await;
    notify(state.ctx(), &id, "task a ended".into());
    settle().await;
    assert_eq!(fake.turns(), 1, "not while the planner is mid-turn");
    assert!(entries_of(&detail(&app, &id).await, "event").is_empty());
    let uri = format!("/api/conversations/{id}/stop");
    call(&app, "POST", &uri, None).await;
    turns_started(&fake, 2).await;
    let d = idle(&app, &id).await;
    assert_eq!(entries_of(&d, "event").len(), 1);
}

#[tokio::test]
async fn an_ended_conversation_is_not_woken() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let state = env.state_with(quick(&fake));
    let app = router(state.clone());
    let id = id_of(&create(&app, "hello").await);
    idle(&app, &id).await;
    call(&app, "POST", &format!("/api/conversations/{id}/end"), None).await;
    notify(state.ctx(), &id, "task a ended".into());
    settle().await;
    assert_eq!(fake.turns(), 1);
}
