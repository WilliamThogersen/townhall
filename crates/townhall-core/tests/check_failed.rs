use townhall_core::model::{
    AgentEvent, Done, EntryBody, Task, TaskState, Verify, VerifyPhase, MAX_HANDOFFS,
};
use townhall_core::Error;

mod common;
use common::{new_task, store_env};

const CODER: &str = "name = \"coder\"\nadapter = \"claude\"\n";

/// A working task under `check` whose agent ended: verifying, `retries` already used.
async fn verifying(env: &common::StoreEnv, max_retries: u32, retries: u32) -> (String, String) {
    env.store.save_agent_type(CODER).await.unwrap();
    let c = env.store.create_conversation().await.unwrap();
    let mut new = new_task("fix");
    new.agent = "coder".into();
    new.done = Done {
        check: Some("npm test".into()),
        max_retries,
        ..Done::default()
    };
    env.store.create_task(&c.id, new).await.unwrap();
    env.store
        .update_task("fix", move |t| {
            Ok(Task {
                state: TaskState::Working,
                done: Done {
                    verify: Some(Verify {
                        phase: VerifyPhase::Verifying,
                        retries,
                        output: None,
                    }),
                    ..t.done
                },
                ..t
            })
        })
        .await
        .unwrap();
    (c.id, "fix".into())
}

#[tokio::test]
async fn a_failed_check_queues_the_task_again_with_the_output_in_its_hand_backs() {
    let env = store_env();
    let (cid, name) = verifying(&env, 2, 0).await;
    let task = env
        .store
        .check_failed(&name, "1 test failed")
        .await
        .unwrap();
    assert_eq!(task.state, TaskState::Queued);
    assert_eq!(task.agent, "coder");
    assert_eq!(
        task.done.verify,
        Some(Verify {
            phase: VerifyPhase::Retrying,
            retries: 1,
            output: Some("1 test failed".into()),
        })
    );
    let handoffs = env.store.handoffs(&name).await.unwrap();
    assert_eq!(handoffs.len(), 1);
    assert!(handoffs[0].reason.contains("1 test failed"), "{handoffs:?}");
    assert!(handoffs[0].reason.contains("retry 1/2"), "{handoffs:?}");
    let last = env.store.entries(&cid).await.unwrap().pop().unwrap().body;
    assert!(matches!(last, EntryBody::Handoff { .. }), "{last:?}");
}

#[tokio::test]
async fn the_last_failed_check_needs_the_user_with_the_output() {
    let env = store_env();
    let (cid, name) = verifying(&env, 2, 2).await;
    let task = env.store.check_failed(&name, "still red").await.unwrap();
    assert_eq!(task.state, TaskState::NeedsYou);
    assert_eq!(
        task.done.verify,
        Some(Verify {
            phase: VerifyPhase::Failed,
            retries: 2,
            output: Some("still red".into()),
        })
    );
    assert!(env.store.handoffs(&name).await.unwrap().is_empty());
    let last = env.store.entries(&cid).await.unwrap().pop().unwrap().body;
    match last {
        EntryBody::Agent { event, text, .. } => {
            assert_eq!(event, AgentEvent::Failed);
            assert!(text.contains("still red"), "{text}");
        }
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn no_retries_means_the_first_failure_needs_the_user() {
    let env = store_env();
    let (_, name) = verifying(&env, 0, 0).await;
    let task = env.store.check_failed(&name, "red").await.unwrap();
    assert_eq!(task.state, TaskState::NeedsYou);
    assert_eq!(task.done.verify.unwrap().phase, VerifyPhase::Failed);
}

#[tokio::test]
async fn a_task_handed_back_the_most_needs_the_user_instead_of_a_retry() {
    let env = store_env();
    let (_, name) = verifying(&env, MAX_HANDOFFS as u32, 0).await;
    for round in 0..MAX_HANDOFFS {
        env.store
            .hand_back(&name, "coder", &format!("round {round}"), "")
            .await
            .unwrap();
        env.store
            .update_task(&name, |t| {
                Ok(Task {
                    state: TaskState::Working,
                    ..t
                })
            })
            .await
            .unwrap();
    }
    let task = env.store.check_failed(&name, "red").await.unwrap();
    assert_eq!(task.state, TaskState::NeedsYou);
}

#[tokio::test]
async fn only_a_task_under_verification_can_have_its_check_fail() {
    let env = store_env();
    let (_, name) = verifying(&env, 2, 0).await;
    env.store
        .update_task(&name, |t| {
            Ok(Task {
                state: TaskState::Ended,
                ..t
            })
        })
        .await
        .unwrap();
    let err = env.store.check_failed(&name, "red").await.unwrap_err();
    assert!(matches!(err, Error::Conflict(_)), "{err:?}");
}
