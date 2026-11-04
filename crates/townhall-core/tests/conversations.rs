use std::time::Duration;

use tokio::sync::broadcast::error::TryRecvError;
use townhall_core::model::{
    AgentEvent, ContextUsage, Conversation, ConversationStatus, EntryBody, RepoRule, ServerEvent,
    Task, TaskState, NEW_CONVERSATION_TITLE,
};
use townhall_core::{ids, Error};

mod common;
use common::{new_task, store_env, you};

async fn tick() {
    tokio::time::sleep(Duration::from_millis(3)).await;
}

#[tokio::test]
async fn create_get_and_list_newest_first() {
    let env = store_env();
    let a = env.store.create_conversation().await.unwrap();
    assert_eq!(a.title, NEW_CONVERSATION_TITLE);
    assert_eq!(a.status, ConversationStatus::YourTurn);
    assert!(ids::is_session_id(&a.session_id));
    assert!(!a.session_started && !a.ended && a.tasks.is_empty() && a.context.is_none());
    tick().await;
    let b = env.store.create_conversation().await.unwrap();
    tick().await;
    env.store.append_entry(&a.id, you("bump")).await.unwrap();
    let ids: Vec<String> = env
        .store
        .conversations()
        .await
        .unwrap()
        .into_iter()
        .map(|c| c.id)
        .collect();
    assert_eq!(ids, vec![a.id.clone(), b.id]);
    assert_eq!(env.store.conversation(&a.id).await.unwrap().id, a.id);
}

#[tokio::test]
async fn update_keeps_identity_and_bumps_time() {
    let env = store_env();
    let c = env.store.create_conversation().await.unwrap();
    tick().await;
    let next = env
        .store
        .update_conversation(&c.id, |c| {
            Ok(Conversation {
                id: "hijack".into(),
                title: "login redirect loop".into(),
                status: ConversationStatus::Thinking,
                linked_repo: Some("shop".into()),
                tasks: vec!["ghost".into()],
                context: Some(ContextUsage {
                    used: 56_000,
                    limit: 200_000,
                }),
                ..c
            })
        })
        .await
        .unwrap();
    assert_eq!(
        (next.id.as_str(), next.created_at.as_str()),
        (c.id.as_str(), c.created_at.as_str())
    );
    assert!(next.updated_at > c.updated_at);
    assert!(next.tasks.is_empty(), "tasks come from the tasks table");
    assert_eq!(env.store.conversation(&c.id).await.unwrap(), next);
}

#[tokio::test]
async fn missing_and_invalid_ids() {
    let env = store_env();
    assert!(env.store.conversations().await.unwrap().is_empty());
    let missing = env.store.conversation("abc").await.unwrap_err();
    assert!(
        matches!(
            missing,
            Error::Missing {
                what: "conversation",
                ..
            }
        ),
        "{missing:?}"
    );
    assert!(matches!(
        env.store.conversation("../x").await,
        Err(Error::InvalidInput(_))
    ));
    assert!(env.store.append_entry("abc", you("x")).await.is_err());
    assert!(env.store.create_task("abc", new_task("t")).await.is_err());
    assert!(env.store.entries("abc").await.is_err());
}

#[tokio::test]
async fn every_entry_kind_round_trips_in_order() {
    let env = store_env();
    let c = env.store.create_conversation().await.unwrap();
    let bodies = vec![
        you("hi"),
        EntryBody::Townhall {
            text: "on it".into(),
        },
        EntryBody::Repo {
            repo: "shop".into(),
            base_branch: "develop".into(),
            rule: RepoRule::Code,
            reason: "it has src/q.ts".into(),
        },
        EntryBody::RepoAsk {
            question: "which one?".into(),
            candidates: vec!["a".into(), "b".into()],
            offer_all: true,
        },
        EntryBody::TasksCreated {
            tasks: vec!["t1".into()],
        },
        EntryBody::NeedsYou {
            agent: "t1".into(),
            question: "go?".into(),
            choices: vec!["y".into(), "n".into()],
        },
        EntryBody::Agent {
            agent: "t1".into(),
            event: AgentEvent::Sent,
            text: "y".into(),
        },
        EntryBody::Error {
            message: "claude exited with 1".into(),
        },
    ];
    for body in &bodies {
        env.store.append_entry(&c.id, body.clone()).await.unwrap();
    }
    let entries = env.store.entries(&c.id).await.unwrap();
    let ids: Vec<u64> = entries.iter().map(|e| e.id).collect();
    assert_eq!(ids, (1..=8).collect::<Vec<u64>>());
    let got: Vec<EntryBody> = entries.into_iter().map(|e| e.body).collect();
    assert_eq!(got, bodies);
}

#[tokio::test]
async fn a_message_only_names_attachments_of_its_conversation() {
    let env = store_env();
    let (a, b) = (
        env.store.create_conversation().await.unwrap(),
        env.store.create_conversation().await.unwrap(),
    );
    let file = env
        .store
        .save_attachment(&b.id, "x.txt", "text/plain", b"x".to_vec())
        .await
        .unwrap();
    let body = EntryBody::You {
        text: "see".into(),
        attachments: vec![file.id.clone()],
    };
    let err = env
        .store
        .append_entry(&a.id, body.clone())
        .await
        .unwrap_err();
    assert!(matches!(err, Error::InvalidInput(_)), "{err:?}");
    assert!(env.store.entries(&a.id).await.unwrap().is_empty());
    assert_eq!(env.store.append_entry(&b.id, body).await.unwrap().id, 1);
}

#[tokio::test]
async fn every_write_publishes_in_order() {
    let env = store_env();
    let mut rx = env.bus.subscribe();
    let c = env.store.create_conversation().await.unwrap();
    env.store.append_entry(&c.id, you("hi")).await.unwrap();
    env.store.create_task(&c.id, new_task("t1")).await.unwrap();
    env.store
        .update_task("t1", |t| {
            Ok(Task {
                state: TaskState::Working,
                ..t
            })
        })
        .await
        .unwrap();
    env.store.update_conversation(&c.id, Ok).await.unwrap();
    let mut kinds = vec![];
    loop {
        match rx.try_recv() {
            Ok(e) => kinds.push(match e {
                ServerEvent::ConversationUpdated { conversation }
                    if conversation.tasks.is_empty() =>
                {
                    "conversation"
                }
                ServerEvent::ConversationUpdated { .. } => "conversation+task",
                ServerEvent::EntryAppended { entry, .. } if entry.id == 1 => "entry",
                ServerEvent::TaskUpdated { task } if task.state == TaskState::Working => "working",
                ServerEvent::TaskUpdated { .. } => "task",
                _ => "other",
            }),
            Err(TryRecvError::Empty) => break,
            Err(e) => panic!("{e}"),
        }
    }
    assert_eq!(
        kinds,
        [
            "conversation",
            "entry",
            "conversation",
            "task",
            "conversation+task",
            "working",
            "conversation+task"
        ]
    );
}
