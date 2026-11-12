use std::time::Duration;

use townhall_core::model::{Conversation, ConversationStatus};
use townhall_core::runner::{notify, RESUME_NOTE};
use townhall_core::ContextOptions;
use townhall_daemon::{router, start_background, AppState};

mod common;
mod runner_support;
use common::Env;
use runner_support::{app_with, create, entries_of, idle, kinds, turns_started, Fake};

fn restart(env: &Env, fake: &Fake, old: &AppState, delay: Duration) -> AppState {
    let options = ContextOptions {
        claude_bin: fake.bin(),
        notify_delay: delay,
        ..ContextOptions::default()
    };
    let store = old.ctx().store().unwrap().clone();
    AppState::with_store(env.workspace(&env.startup), options, store)
}

async fn mark_thinking(state: &AppState, id: &str) {
    let thinking = |c: Conversation| {
        Ok(Conversation {
            status: ConversationStatus::Thinking,
            ..c
        })
    };
    let store = state.ctx().store().unwrap();
    store.update_conversation(id, thinking).await.unwrap();
}

#[tokio::test]
async fn an_interrupted_reply_resumes_by_itself_and_only_once() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (state, app) = app_with(&env, &fake);
    let background = start_background(&state).await;
    let created = create(&app, "SLOW").await;
    let id = created["conversation"]["id"].as_str().unwrap().to_string();
    turns_started(&fake, 1).await;
    background.stop();

    let second = restart(&env, &fake, &state, Duration::from_secs(60));
    let again = start_background(&second).await;
    let d = idle(&router(second.clone()), &id).await;
    again.stop();
    assert!(entries_of(&d, "error").is_empty(), "{d}");
    assert_eq!(fake.turns(), 2);
    let sent = fake.stdin(2)[0].to_string();
    assert!(sent.contains(RESUME_NOTE), "{sent}");
    assert!(fake.argv(2).iter().any(|a| a.starts_with("--resume=")));
    assert_eq!(entries_of(&d, "you").len(), 1, "the note is not a message");
    assert_eq!(kinds(&d).last().map(String::as_str), Some("townhall"));

    mark_thinking(&second, &id).await;
    let third = restart(&env, &fake, &second, Duration::from_secs(60));
    let last = start_background(&third).await;
    let d = idle(&router(third.clone()), &id).await;
    last.stop();
    assert_eq!(fake.turns(), 2, "a second restart does not resume it again");
    let errors = entries_of(&d, "error");
    assert_eq!(errors.len(), 1, "{d}");
    let message = errors[0]["message"].as_str().unwrap();
    assert!(message.starts_with("townhall restarted"), "{message}");
}

#[tokio::test]
async fn a_task_event_not_yet_delivered_is_delivered_after_a_restart() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (state, app) = app_with(&env, &fake);
    let id = create(&app, "hello").await["conversation"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    idle(&app, &id).await;
    let slow = restart(&env, &fake, &state, Duration::from_secs(600));
    notify(slow.ctx(), &id, "task a ended".into());
    let store = slow.ctx().store().unwrap().clone();
    for _ in 0..250 {
        if !store.pending_events().await.unwrap().is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(store.pending_events().await.unwrap().len(), 1);
    assert_eq!(fake.turns(), 1);

    let second = restart(&env, &fake, &slow, Duration::from_millis(100));
    let background = start_background(&second).await;
    turns_started(&fake, 2).await;
    let d = idle(&router(second.clone()), &id).await;
    background.stop();
    let events = entries_of(&d, "event");
    assert_eq!(events.len(), 1, "{d}");
    assert_eq!(events[0]["text"], "task a ended");
    assert!(store.pending_events().await.unwrap().is_empty());
}
