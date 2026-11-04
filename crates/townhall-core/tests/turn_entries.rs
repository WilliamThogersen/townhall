use townhall_core::model::{EntryBody, ServerEvent};
use townhall_core::store::Store;

mod common;
use common::{store_env, you};

async fn tasks_created(store: &Store, id: &str) -> Vec<(u64, Vec<String>)> {
    let entries = store.entries(id).await.unwrap();
    entries
        .into_iter()
        .filter_map(|e| match e.body {
            EntryBody::TasksCreated { tasks } => Some((e.id, tasks)),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn a_turn_starts_at_your_latest_message() {
    let env = store_env();
    let c = env.store.create_conversation().await.unwrap();
    let first = env.store.note_task_created(&c.id, "a").await.unwrap();
    env.store
        .append_entry(
            &c.id,
            EntryBody::Townhall {
                text: "on it".into(),
            },
        )
        .await
        .unwrap();
    let mut events = env.bus.subscribe();
    let merged = env.store.note_task_created(&c.id, "b").await.unwrap();
    assert_eq!((merged.id, &merged.at), (first.id, &first.at));
    assert_eq!(
        merged.body,
        EntryBody::TasksCreated {
            tasks: vec!["a".into(), "b".into()]
        }
    );
    assert_eq!(
        events.recv().await.unwrap(),
        ServerEvent::EntryAppended {
            conversation_id: c.id.clone(),
            entry: merged.clone()
        }
    );
    let again = env.store.note_task_created(&c.id, "b").await.unwrap();
    assert_eq!(again.body, merged.body, "a name is listed once");

    env.store.append_entry(&c.id, you("next")).await.unwrap();
    let next = env.store.note_task_created(&c.id, "c").await.unwrap();
    assert!(next.id > first.id);
    assert_eq!(
        tasks_created(&env.store, &c.id).await,
        vec![
            (first.id, vec!["a".to_string(), "b".to_string()]),
            (next.id, vec!["c".to_string()])
        ]
    );
}

#[tokio::test]
async fn concurrent_tasks_of_one_turn_share_one_entry() {
    let env = store_env();
    let c = env.store.create_conversation().await.unwrap();
    env.store.append_entry(&c.id, you("go")).await.unwrap();
    let handles: Vec<_> = (0..6)
        .map(|i| {
            let (store, id) = (env.store.clone(), c.id.clone());
            tokio::spawn(async move { store.note_task_created(&id, &format!("t{i}")).await })
        })
        .collect();
    for h in handles {
        h.await.unwrap().unwrap();
    }
    let created = tasks_created(&env.store, &c.id).await;
    assert_eq!(created.len(), 1, "{created:?}");
    let mut names = created[0].1.clone();
    names.sort();
    assert_eq!(names, ["t0", "t1", "t2", "t3", "t4", "t5"]);
}

#[tokio::test]
async fn an_unknown_conversation_is_missing() {
    let env = store_env();
    let err = env
        .store
        .note_task_created("nope00000000", "a")
        .await
        .unwrap_err();
    assert!(
        matches!(err, townhall_core::Error::Missing { .. }),
        "{err:?}"
    );
}
