use townhall_core::model::{EntryBody, TaskState, MAX_HANDOFFS};
use townhall_core::Error;

mod common;
use common::{new_task, store_env};

const CODER: &str = "name = \"coder\"\nadapter = \"claude\"\n";
const SHIPPER: &str = "name = \"shipper\"\nadapter = \"claude\"\n";

async fn running(env: &common::StoreEnv, name: &str, agent: &str) -> (String, String) {
    env.store.save_agent_type(CODER).await.unwrap();
    env.store.save_agent_type(SHIPPER).await.unwrap();
    let c = env.store.create_conversation().await.unwrap();
    let mut new = new_task(name);
    new.agent = agent.into();
    env.store.create_task(&c.id, new).await.unwrap();
    env.store
        .update_task(name, |t| {
            Ok(townhall_core::model::Task {
                state: TaskState::Working,
                ..t
            })
        })
        .await
        .unwrap();
    (c.id, name.into())
}

#[tokio::test]
async fn a_hand_back_requeues_the_task_as_the_target_type_and_shows_in_the_thread() {
    let env = store_env();
    let (cid, name) = running(&env, "fix-login", "shipper").await;
    let handoff = env
        .store
        .hand_back(&name, "Coder", "ci fails on lint", "opened the pr")
        .await
        .unwrap();
    assert_eq!(
        (handoff.from.as_str(), handoff.to.as_str()),
        ("shipper", "coder")
    );
    let task = env.store.find_task(&name).await.unwrap().unwrap();
    assert_eq!(
        (task.state, task.agent.as_str()),
        (TaskState::Queued, "coder")
    );
    assert_eq!(task.prompt, "the brief");
    assert_eq!(env.store.handoffs(&name).await.unwrap(), vec![handoff]);
    let last = env.store.entries(&cid).await.unwrap().pop().unwrap().body;
    assert_eq!(
        last,
        EntryBody::Handoff {
            task: name,
            from: "shipper".into(),
            to: "coder".into(),
            reason: "ci fails on lint".into(),
        }
    );
}

#[tokio::test]
async fn only_a_running_task_can_be_handed_back_to_a_saved_type() {
    let env = store_env();
    let (_, name) = running(&env, "fix-login", "shipper").await;
    let unknown = env
        .store
        .hand_back(&name, "ghost", "why", "")
        .await
        .unwrap_err();
    assert!(
        matches!(&unknown, Error::InvalidInput(m) if m.contains("ghost") && m.contains("coder")),
        "{unknown:?}"
    );
    env.store
        .update_task(&name, |t| {
            Ok(townhall_core::model::Task {
                state: TaskState::Ended,
                ..t
            })
        })
        .await
        .unwrap();
    let ended = env
        .store
        .hand_back(&name, "coder", "why", "")
        .await
        .unwrap_err();
    assert!(matches!(ended, Error::Conflict(_)), "{ended:?}");
    assert!(env.store.handoffs(&name).await.unwrap().is_empty());
}

#[tokio::test]
async fn an_ended_task_is_handed_on_and_a_running_one_is_not() {
    let env = store_env();
    let (_, name) = running(&env, "fix-login", "shipper").await;
    let running = env
        .store
        .hand_on(&name, "coder", "why", "")
        .await
        .unwrap_err();
    assert!(matches!(running, Error::Conflict(_)), "{running:?}");
    env.store
        .update_task(&name, |t| {
            Ok(townhall_core::model::Task {
                state: TaskState::Ended,
                ..t
            })
        })
        .await
        .unwrap();
    let handoff = env
        .store
        .hand_on(&name, "coder", "why", "done")
        .await
        .unwrap();
    assert_eq!(
        (handoff.from.as_str(), handoff.to.as_str()),
        ("shipper", "coder")
    );
    let task = env.store.find_task(&name).await.unwrap().unwrap();
    assert_eq!(
        (task.state, task.agent.as_str()),
        (TaskState::Queued, "coder")
    );
}

#[tokio::test]
async fn a_chain_stops_after_the_most_handoffs() {
    let env = store_env();
    let (_, name) = running(&env, "fix-login", "shipper").await;
    for round in 0..MAX_HANDOFFS {
        env.store
            .hand_back(&name, "coder", &format!("round {round}"), "")
            .await
            .unwrap();
        env.store
            .update_task(&name, |t| {
                Ok(townhall_core::model::Task {
                    state: TaskState::Working,
                    ..t
                })
            })
            .await
            .unwrap();
    }
    let stopped = env
        .store
        .hand_back(&name, "shipper", "again", "")
        .await
        .unwrap_err();
    assert!(
        matches!(&stopped, Error::Conflict(m) if m.contains(&MAX_HANDOFFS.to_string())),
        "{stopped:?}"
    );
    assert_eq!(env.store.handoffs(&name).await.unwrap().len(), MAX_HANDOFFS);
}
