use townhall_core::model::{Checks, Conversation, EntryBody, Pr, Task, TaskState};
use townhall_core::store::{HistoryHit, HistoryKind, HistoryQuery};
use townhall_core::Error;

mod common;
use common::{new_task, store_env, you, StoreEnv};

fn query(text: &str) -> HistoryQuery {
    HistoryQuery {
        query: text.into(),
        repo: None,
        since_ms: None,
        kinds: vec![],
        limit: 10,
    }
}

async fn find(env: &StoreEnv, q: HistoryQuery) -> Vec<HistoryHit> {
    env.store.search_history(q).await.unwrap()
}

async fn say(env: &StoreEnv, conversation: &str, text: &str) {
    env.store
        .append_entry(conversation, you(text))
        .await
        .unwrap();
}

async fn link(env: &StoreEnv, id: &str, repo: &str) {
    let repo = repo.to_string();
    env.store
        .update_conversation(id, move |c| {
            Ok(Conversation {
                linked_repo: Some(repo),
                ..c
            })
        })
        .await
        .unwrap();
}

async fn titled(env: &StoreEnv, title: &str) -> Conversation {
    let c = env.store.create_conversation().await.unwrap();
    let title = title.to_string();
    env.store
        .update_conversation(&c.id, move |c| Ok(Conversation { title, ..c }))
        .await
        .unwrap()
}

#[tokio::test]
async fn hits_are_ranked_by_bm25_and_carry_a_snippet_and_the_conversation() {
    let env = store_env();
    let a = titled(&env, "release plan").await;
    let b = titled(&env, "chores").await;
    say(&env, &a.id, "we should deploy deploy deploy on friday").await;
    say(
        &env,
        &b.id,
        "a long note about many unrelated things, the lunch menu, the weather, the office plants, and once the deploy",
    )
    .await;
    say(&env, &b.id, "nothing relevant at all").await;

    let hits = find(&env, query("deploy")).await;
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].conversation_id, a.id);
    assert_eq!(hits[0].title, "release plan");
    assert_eq!(hits[0].kind, HistoryKind::Message);
    assert_eq!(hits[1].conversation_id, b.id);
    assert!(hits[0].snippet.contains("[deploy]"), "{}", hits[0].snippet);
    assert!(hits[1].snippet.contains("[deploy]"), "{}", hits[1].snippet);
    assert!(hits[1].snippet.chars().count() < 160, "{}", hits[1].snippet);
    assert!(!hits[1].snippet.contains('\n'));
    assert!(hits[0].task.is_none() && hits[0].pr_number.is_none());
}

#[tokio::test]
async fn words_are_stemmed_and_hyphenated_words_are_plain_words() {
    let env = store_env();
    let c = env.store.create_conversation().await.unwrap();
    say(&env, &c.id, "we deployed the fix-flaky-timing-tests branch").await;
    for q in [
        "deploying",
        "fix-flaky-timing-tests",
        "flaky timing",
        "DEPLOYED",
    ] {
        assert_eq!(find(&env, query(q)).await.len(), 1, "{q}");
    }
    assert!(find(&env, query("deploy rollback")).await.is_empty());
    assert!(find(&env, query("   ")).await.is_empty());
}

#[tokio::test]
async fn fts_syntax_is_passed_through_and_bad_syntax_is_refused() {
    let env = store_env();
    let c = env.store.create_conversation().await.unwrap();
    say(&env, &c.id, "alpha beta").await;
    say(&env, &c.id, "gamma delta").await;
    assert_eq!(find(&env, query("alpha OR gamma")).await.len(), 2);
    assert_eq!(find(&env, query("alp*")).await.len(), 1);
    assert_eq!(find(&env, query("\"alpha beta\"")).await.len(), 1);
    assert_eq!(find(&env, query("alpha NOT beta")).await.len(), 0);
    let err = env
        .store
        .search_history(query("\"unterminated"))
        .await
        .unwrap_err();
    assert!(matches!(err, Error::InvalidInput(_)), "{err:?}");
    assert!(err.to_string().contains("double quotes"), "{err}");
}

#[tokio::test]
async fn tasks_and_agent_reports_carry_their_task_agent_state_branch_and_pr() {
    let env = store_env();
    let c = titled(&env, "login work").await;
    env.store
        .create_task(&c.id, new_task("fix-login"))
        .await
        .unwrap();
    env.store
        .update_task("fix-login", |t| {
            Ok(Task {
                state: TaskState::Merged,
                pr: Some(Pr {
                    number: 42,
                    url: Some("https://git.example/shop/pull/42".into()),
                    checks: Checks::Pending,
                    merge_sha: None,
                }),
                ..t
            })
        })
        .await
        .unwrap();
    let report = EntryBody::Event {
        text: "task fix-login ended (done). its agent's last message: rejected the oauth rewrite, too risky".into(),
    };
    env.store.append_entry(&c.id, report).await.unwrap();
    let asks = EntryBody::NeedsYou {
        agent: "fix-login".into(),
        question: "keep the oauth provider?".into(),
        choices: vec![],
    };
    env.store.append_entry(&c.id, asks).await.unwrap();

    let hits = find(&env, query("oauth")).await;
    assert_eq!(hits.len(), 2);
    let kinds: Vec<HistoryKind> = hits.iter().map(|h| h.kind).collect();
    assert!(kinds.contains(&HistoryKind::AgentReport) && kinds.contains(&HistoryKind::NeedsYou));
    for hit in &hits {
        assert_eq!(hit.task.as_deref(), Some("fix-login"));
        assert_eq!(hit.agent.as_deref(), Some("builder"));
        assert_eq!(hit.state, Some(TaskState::Merged));
        assert_eq!(hit.branch.as_deref(), Some("townhall/fix-login"));
        assert_eq!(hit.pr_number, Some(42));
        assert_eq!(
            hit.pr_url.as_deref(),
            Some("https://git.example/shop/pull/42")
        );
        assert_eq!(hit.title, "login work");
    }

    let task = find(&env, query("brief")).await;
    assert_eq!(task.len(), 1);
    assert_eq!(task[0].kind, HistoryKind::Task);
    assert_eq!(task[0].task.as_deref(), Some("fix-login"));
}

#[tokio::test]
async fn kind_since_repo_and_limit_filters_narrow_the_hits() {
    let env = store_env();
    let shop = titled(&env, "shop").await;
    let admin = titled(&env, "admin").await;
    link(&env, &shop.id, "shop").await;
    link(&env, &admin.id, "admin").await;
    say(&env, &shop.id, "billing bug in the cart").await;
    say(&env, &admin.id, "billing bug in the panel").await;
    env.store
        .create_task(&shop.id, new_task("cart-fix"))
        .await
        .unwrap();
    env.store
        .update_task("cart-fix", |t| {
            Ok(Task {
                title: "billing cart fix".into(),
                ..t
            })
        })
        .await
        .unwrap();

    assert_eq!(find(&env, query("billing")).await.len(), 3);

    let only_tasks = HistoryQuery {
        kinds: vec![HistoryKind::Task],
        ..query("billing")
    };
    let hits = find(&env, only_tasks).await;
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].kind, HistoryKind::Task);

    let in_admin = HistoryQuery {
        repo: Some("ADMIN".into()),
        ..query("billing")
    };
    let hits = find(&env, in_admin).await;
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].conversation_id, admin.id);

    let in_shop = HistoryQuery {
        repo: Some("shop".into()),
        ..query("billing")
    };
    assert_eq!(find(&env, in_shop).await.len(), 2);

    let one = HistoryQuery {
        limit: 1,
        ..query("billing")
    };
    assert_eq!(find(&env, one).await.len(), 1);

    let future = HistoryQuery {
        since_ms: Some(townhall_core::clock::unix_millis(std::time::SystemTime::now()) + 60_000),
        ..query("billing")
    };
    assert!(find(&env, future).await.is_empty());
    let past = HistoryQuery {
        since_ms: Some(0),
        ..query("billing")
    };
    assert_eq!(find(&env, past).await.len(), 3);
}
