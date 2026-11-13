use rusqlite::Connection;
use townhall_core::model::{EntryBody, TaskState};
use townhall_core::store::migrate;

mod common;
use common::{new_task, store_env, you};

const FIRST_HISTORY_MIGRATION: u32 = 1000;

fn count(conn: &Connection, kind: &str) -> i64 {
    conn.query_row(
        "select count(*) from history_fts where kind = ?1",
        [kind],
        |r| r.get(0),
    )
    .unwrap()
}

fn text_of(conn: &Connection, kind: &str) -> Vec<String> {
    let mut stmt = conn
        .prepare("select text from history_fts where kind = ?1 order by text")
        .unwrap();
    let rows = stmt.query_map([kind], |r| r.get::<_, String>(0)).unwrap();
    rows.map(Result::unwrap).collect()
}

fn event_text(task: &str, last: &str) -> EntryBody {
    EntryBody::Event {
        text: format!("task {task} ended (done). its agent's last message: {last}"),
    }
}

#[test]
fn the_migration_indexes_the_rows_that_exist_before_it() {
    let mut conn = Connection::open_in_memory().unwrap();
    let before: Vec<_> = migrate::all()
        .into_iter()
        .filter(|m| m.id < FIRST_HISTORY_MIGRATION)
        .collect();
    migrate::apply(&mut conn, &before).unwrap();
    conn.execute_batch(
        "insert into conversations (id, session_id, title, status, created_at, updated_at)
           values ('c1', 's1', 'login', 'idle', '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z');
         insert into messages (conversation_id, id, author, text, at)
           values ('c1', 1, 'you', 'please fix the login page', '2026-01-01T00:00:01.000Z'),
                  ('c1', 2, 'townhall', 'on it', '2026-01-01T00:00:02.000Z');
         insert into events (conversation_id, id, kind, data, at) values
           ('c1', 3, 'needsYou', '{\"agent\":\"fix-login\",\"question\":\"which provider?\",\"choices\":[]}', '2026-01-01T00:00:03.000Z'),
           ('c1', 4, 'event', '{\"text\":\"task fix-login ended (done). its agent''s last message: all fixed\"}', '2026-01-01T00:00:04.000Z'),
           ('c1', 5, 'event', '{\"text\":\"task fix-login needs you, its agent asks: which provider?\"}', '2026-01-01T00:00:05.000Z'),
           ('c1', 6, 'repo', '{\"repo\":\"shop\"}', '2026-01-01T00:00:06.000Z');
         insert into tasks (name, conversation_id, title, prompt, repo, base_branch, state, created_at)
           values ('fix-login', 'c1', 'Fix login', 'the login page is broken', 'shop', 'main', 'ended', '2026-01-01T00:00:07.000Z');",
    )
    .unwrap();
    assert_eq!(migrate::apply(&mut conn, &migrate::all()).unwrap(), 7);
    assert_eq!(count(&conn, "message"), 2);
    assert_eq!(count(&conn, "needs_you"), 1);
    assert_eq!(count(&conn, "agent_report"), 1);
    assert_eq!(count(&conn, "task"), 1);
    assert_eq!(
        text_of(&conn, "task"),
        ["Fix login\nthe login page is broken"]
    );
    let task: String = conn
        .query_row(
            "select task from history_fts where kind = 'agent_report'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(task, "fix-login");
}

#[tokio::test]
async fn the_index_follows_inserts_updates_and_deletes() {
    let env = store_env();
    let store = &env.store;
    let c = store.create_conversation().await.unwrap();
    store
        .append_entry(&c.id, you("fix the login"))
        .await
        .unwrap();
    let reply = EntryBody::Townhall { text: "ok".into() };
    store.append_entry(&c.id, reply).await.unwrap();
    store
        .create_task(&c.id, new_task("fix-login"))
        .await
        .unwrap();
    let asks = EntryBody::NeedsYou {
        agent: "fix-login".into(),
        question: "which provider?".into(),
        choices: vec![],
    };
    store.append_entry(&c.id, asks).await.unwrap();
    store
        .append_entry(&c.id, event_text("fix-login", "all fixed"))
        .await
        .unwrap();
    let plain = EntryBody::Event {
        text: "townhall event: something else".into(),
    };
    store.append_entry(&c.id, plain).await.unwrap();

    let counts = store
        .call(|conn| Ok(["message", "task", "needs_you", "agent_report"].map(|k| count(conn, k))))
        .await
        .unwrap();
    assert_eq!(counts, [2, 1, 1, 1]);

    store
        .update_task("fix-login", |t| {
            Ok(townhall_core::model::Task {
                title: "renamed".into(),
                state: TaskState::Working,
                ..t
            })
        })
        .await
        .unwrap();
    let tasks = store.call(|conn| Ok(text_of(conn, "task"))).await.unwrap();
    assert_eq!(tasks, ["renamed\nthe brief"]);

    store
        .call(|conn| {
            conn.execute("update messages set text = 'changed words' where id = 1", [])?;
            conn.execute("update events set data = json_set(data, '$.question', 'which colour?') where kind = 'needsYou'", [])?;
            Ok(())
        })
        .await
        .unwrap();
    let (messages, asks) = store
        .call(|conn| Ok((text_of(conn, "message"), text_of(conn, "needs_you"))))
        .await
        .unwrap();
    assert_eq!(messages, ["changed words", "ok"]);
    assert_eq!(asks, ["which colour?"]);

    store
        .call(|conn| {
            conn.execute("delete from messages where id = 2", [])?;
            conn.execute("delete from events where kind = 'needsYou'", [])?;
            conn.execute("delete from tasks", [])?;
            Ok(())
        })
        .await
        .unwrap();
    let left = store
        .call(|conn| Ok(["message", "task", "needs_you", "agent_report"].map(|k| count(conn, k))))
        .await
        .unwrap();
    assert_eq!(left, [1, 0, 0, 1]);
}

#[tokio::test]
async fn deleting_a_conversation_leaves_nothing_of_it_in_the_index() {
    let env = store_env();
    let store = &env.store;
    let (a, b) = (
        store.create_conversation().await.unwrap(),
        store.create_conversation().await.unwrap(),
    );
    for c in [&a, &b] {
        store.append_entry(&c.id, you("hello there")).await.unwrap();
    }
    store.create_task(&a.id, new_task("a-task")).await.unwrap();
    let id = a.id.clone();
    let left = store
        .call(move |conn| {
            conn.execute("delete from conversations where id = ?1", [&id])?;
            Ok(conn.query_row("select count(*) from history_fts", [], |r| {
                r.get::<_, i64>(0)
            })?)
        })
        .await
        .unwrap();
    assert_eq!(left, 1);
}
