use axum::http::StatusCode;
use serde_json::json;
use townhall_core::mcp;

mod common;
mod runner_support;
use common::{call, Env};
use runner_support::{
    app_with, create, detail, entries_of, idle, kinds, message, turns_started, values_of, Fake,
};

const FIRST_FLAGS: [&str; 6] = [
    "-p",
    "--output-format=stream-json",
    "--verbose",
    "--input-format=stream-json",
    "--permission-prompt-tool=stdio",
    "--include-partial-messages",
];

/// The flags of a first turn: the session id, no bypass, townhall's tools only.
fn check_first_turn(first: &[String], session: &str, config: String) {
    assert_eq!(first[..6], FIRST_FLAGS);
    assert!(first[1..].iter().all(|a| a.starts_with("--")), "{first:?}");
    assert_eq!(values_of(first, "--session-id"), [session]);
    assert!(values_of(first, "--resume").is_empty(), "{first:?}");
    assert_eq!(values_of(first, "--permission-mode"), ["default"]);
    assert_eq!(values_of(first, "--model"), ["opus"]);
    assert_eq!(values_of(first, "--mcp-config"), [config]);
    assert_eq!(
        values_of(first, "--tools"),
        ["Read,Grep,Glob,WebFetch,WebSearch"]
    );
    assert_eq!(
        values_of(first, "--allowedTools"),
        ["mcp__townhall__create_task,mcp__townhall__set_title,mcp__townhall__task_status,mcp__townhall__list_agent_types,mcp__townhall__read_agent_type,mcp__townhall__save_agent_type,mcp__townhall__read_logs,mcp__townhall__log_summary,mcp__townhall__conversation_insights,Read,Grep,Glob,WebFetch,WebSearch"]
    );
    assert_eq!(values_of(first, "--setting-sources"), [""]);
    assert_eq!(
        values_of(first, "--settings"),
        [r#"{"disableAllHooks":true,"autoMemoryEnabled":false}"#]
    );
    assert!(first.contains(&"--disable-slash-commands".to_string()));
    assert!(!first.iter().any(|a| a == "--bare" || a == "--safe-mode"));
    assert_eq!(
        values_of(first, "--disallowedTools"),
        ["Bash,Edit,MultiEdit,Write,NotebookEdit"]
    );
    assert!(first.contains(&"--strict-mcp-config".to_string()));
    assert!(!first.iter().any(|a| a.contains("dangerously")));
    let prompt = values_of(first, "--append-system-prompt").join(" ");
    assert!(prompt.contains('\n'), "the system prompt keeps its lines");
    assert!(prompt.contains("create_task") && prompt.contains("set_title"));
    assert!(prompt.contains("task_status"));
    assert!(prompt.contains("alpha"), "{prompt}");
}

#[tokio::test]
async fn the_first_turn_starts_the_session_and_later_ones_resume_it() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (state, app) = app_with(&env, &fake);
    let created = create(&app, "plan a settings page").await;
    let id = created["conversation"]["id"].as_str().unwrap().to_string();
    let session = created["conversation"]["sessionId"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(created["conversation"]["status"], "thinking");
    assert_eq!(created["entries"][0]["kind"], "you");
    assert_eq!(created["entries"][0]["text"], "plan a settings page");

    let d = idle(&app, &id).await;
    assert_eq!(d["conversation"]["sessionStarted"], true);
    let config = mcp::config_json(state.ctx().base_url(), &id);
    check_first_turn(&fake.argv(1), &session, config);
    assert_eq!(fake.cwd(1), env.startup);

    let (s, v) = message(&app, &id, "and a dark theme").await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["data"]["kind"], "you");
    idle(&app, &id).await;
    let second = fake.argv(2);
    assert_eq!(values_of(&second, "--resume"), [session]);
    assert!(values_of(&second, "--session-id").is_empty(), "{second:?}");
    assert_eq!(fake.cwd(2), env.startup);
}

#[tokio::test]
async fn the_default_types_model_picks_the_model_of_the_next_turn() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (state, app) = app_with(&env, &fake);
    let store = state.ctx().store().unwrap();
    let typed =
        |model: &str| format!("name = \"default\"\nadapter = \"claude\"\nmodel = \"{model}\"\n");
    store.save_agent_type(&typed("haiku")).await.unwrap();
    let created = create(&app, "plan a settings page").await;
    let id = created["conversation"]["id"].as_str().unwrap().to_string();
    idle(&app, &id).await;
    assert_eq!(values_of(&fake.argv(1), "--model"), ["haiku"]);
    store.save_agent_type(&typed("sonnet")).await.unwrap();
    let (s, v) = message(&app, &id, "and a dark theme").await;
    assert_eq!(s, StatusCode::OK, "{v}");
    idle(&app, &id).await;
    let second = fake.argv(2);
    assert_eq!(values_of(&second, "--model"), ["sonnet"]);
    assert_eq!(values_of(&second, "--setting-sources"), [""]);
    store
        .save_agent_type("name = \"default\"\nadapter = \"claude\"\nmodel = \" \"\n")
        .await
        .unwrap();
    let (s, v) = message(&app, &id, "and a light theme").await;
    assert_eq!(s, StatusCode::OK, "{v}");
    idle(&app, &id).await;
    assert_eq!(values_of(&fake.argv(3), "--model"), ["opus"]);
}

#[tokio::test]
async fn a_fresh_install_runs_conversations_on_opus() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (_state, app) = app_with(&env, &fake);
    let created = create(&app, "plan a settings page").await;
    let id = created["conversation"]["id"].as_str().unwrap().to_string();
    idle(&app, &id).await;
    assert_eq!(values_of(&fake.argv(1), "--model"), ["opus"]);
}

const EDITED_DEFAULT: &str = "name = \"default\"\nadapter = \"claude\"\nmodel = \"opus\"\npermission_mode = \"acceptEdits\"\ntools = [\"Read\", \"Bash\", \"mcp__townhall__create_task\"]\nprompt = \"you are a pirate planner\"\n";

#[tokio::test]
async fn the_default_agent_type_drives_the_conversation() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (state, app) = app_with(&env, &fake);
    let store = state.ctx().store().unwrap();
    let (_, replaced) = store.save_agent_type(EDITED_DEFAULT).await.unwrap();
    assert!(replaced, "the type default is seeded");
    let created = create(&app, "plan a settings page").await;
    let id = created["conversation"]["id"].as_str().unwrap().to_string();
    idle(&app, &id).await;
    let first = fake.argv(1);
    assert_eq!(values_of(&first, "--model"), ["opus"]);
    assert_eq!(values_of(&first, "--permission-mode"), ["acceptEdits"]);
    assert_eq!(values_of(&first, "--tools"), ["Read"]);
    assert_eq!(
        values_of(&first, "--allowedTools"),
        ["Read,mcp__townhall__create_task"]
    );
    assert_eq!(
        values_of(&first, "--disallowedTools"),
        ["Bash,Edit,MultiEdit,Write,NotebookEdit"]
    );
    let prompt = values_of(&first, "--append-system-prompt").join(" ");
    assert!(
        prompt.starts_with("you are a pirate planner\n\n"),
        "{prompt}"
    );
    assert!(prompt.contains("alpha") && !prompt.contains("you are townhall"));

    store
        .save_agent_type(&EDITED_DEFAULT.replace("acceptEdits", "plan"))
        .await
        .unwrap();
    let (s, v) = message(&app, &id, "and a dark theme").await;
    assert_eq!(s, StatusCode::OK, "{v}");
    idle(&app, &id).await;
    assert_eq!(values_of(&fake.argv(2), "--permission-mode"), ["plan"]);
}

#[tokio::test]
async fn a_turn_stores_the_reply_and_the_context_but_no_deltas() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (_state, app) = app_with(&env, &fake);
    let id = create(&app, "hello").await["conversation"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let d = idle(&app, &id).await;
    assert_eq!(kinds(&d), ["you", "repo", "townhall"]);
    assert_eq!(entries_of(&d, "townhall")[0]["text"], "on it.");
    assert_eq!(
        d["conversation"]["context"],
        json!({"used": 2000, "limit": 200000})
    );
    let user = &fake.stdin(1)[0];
    assert_eq!(user["type"], "user");
    assert_eq!(
        user["message"]["content"],
        json!([{"type": "text", "text": "hello"}])
    );
    let (s, v) = call(&app, "GET", "/api/conversations", None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v["data"][0]["id"], id);
}

#[tokio::test]
async fn a_second_message_while_thinking_is_a_conflict_and_stop_interrupts() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (_state, app) = app_with(&env, &fake);
    let id = create(&app, "SLOW one").await["conversation"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    turns_started(&fake, 1).await;
    let (s, v) = message(&app, &id, "two").await;
    assert_eq!(s, StatusCode::CONFLICT, "{v}");
    assert_eq!(
        detail(&app, &id).await["conversation"]["status"],
        "thinking"
    );

    let (s, v) = call(&app, "POST", &format!("/api/conversations/{id}/stop"), None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["data"]["status"], "yourTurn");
    let interrupts: Vec<_> = fake
        .stdin(1)
        .into_iter()
        .filter(|l| l["request"]["subtype"] == "interrupt")
        .collect();
    assert_eq!(interrupts.len(), 1);
    assert_eq!(interrupts[0]["type"], "control_request");
    let d = idle(&app, &id).await;
    assert!(
        entries_of(&d, "error").is_empty(),
        "a stop is no error: {d}"
    );

    let (s, _) = message(&app, &id, "three").await;
    assert_eq!(s, StatusCode::OK);
    idle(&app, &id).await;
    assert_eq!(fake.turns(), 2);
}

#[tokio::test]
async fn stop_kills_a_turn_that_ignores_the_interrupt() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (_state, app) = app_with(&env, &fake);
    let id = create(&app, "HANG").await["conversation"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    turns_started(&fake, 1).await;
    let (s, v) = call(&app, "POST", &format!("/api/conversations/{id}/stop"), None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["data"]["status"], "yourTurn");
    let (s, _) = message(&app, &id, "again").await;
    assert_eq!(s, StatusCode::OK);
    idle(&app, &id).await;
}

#[tokio::test]
async fn townhall_denies_tools_outside_its_list() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (_state, app) = app_with(&env, &fake);
    let id = create(&app, "TOOL please").await["conversation"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    idle(&app, &id).await;
    let answers: Vec<_> = fake
        .stdin(1)
        .into_iter()
        .filter(|l| l["type"] == "control_response")
        .collect();
    assert_eq!(answers.len(), 2, "{answers:?}");
    let edit = &answers[0]["response"];
    assert_eq!(edit["subtype"], "success");
    assert_eq!(edit["request_id"], "req-edit");
    assert_eq!(edit["response"]["behavior"], "deny");
    let why = edit["response"]["message"].as_str().unwrap();
    assert!(why.contains("Edit") && why.contains("create_task"), "{why}");
    let read = &answers[1]["response"];
    assert_eq!(read["request_id"], "req-read");
    assert_eq!(read["response"]["behavior"], "allow");
    assert_eq!(read["response"]["updatedInput"], json!({"file_path": "/x"}));
}

#[tokio::test]
async fn a_crash_is_an_error_entry_and_the_session_starts_again() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (_state, app) = app_with(&env, &fake);
    let created = create(&app, "CRASH now").await;
    let id = created["conversation"]["id"].as_str().unwrap().to_string();
    let session = created["conversation"]["sessionId"]
        .as_str()
        .unwrap()
        .to_string();
    let d = idle(&app, &id).await;
    let errors = entries_of(&d, "error");
    assert_eq!(errors.len(), 1, "{d}");
    assert_eq!(errors[0]["message"], "claude failed: boom: claude broke");
    assert_eq!(d["conversation"]["sessionStarted"], false);
    message(&app, &id, "try again").await;
    idle(&app, &id).await;
    assert_eq!(values_of(&fake.argv(2), "--session-id"), [session]);
}

#[tokio::test]
async fn a_missing_claude_is_an_error_entry() {
    let env = Env::new("alpha").await;
    let options = townhall_core::ContextOptions {
        claude_bin: env.root.path().join("no-claude").to_string_lossy().into(),
        ..townhall_core::ContextOptions::default()
    };
    let app = townhall_daemon::router(env.state_with(options));
    let id = create(&app, "hello").await["conversation"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let d = idle(&app, &id).await;
    let message = entries_of(&d, "error")[0]["message"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(message.starts_with("cannot run claude"), "{message}");
}

#[tokio::test]
async fn patch_sets_the_title_and_the_linked_repo() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (_state, app) = app_with(&env, &fake);
    let id = create(&app, "hello").await["conversation"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    idle(&app, &id).await;
    let uri = format!("/api/conversations/{id}");
    let (s, v) = call(&app, "PATCH", &uri, Some(json!({"title": "login loop"}))).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["data"]["title"], "login loop");
    let (s, v) = call(&app, "PATCH", &uri, Some(json!({"linkedRepo": "ALPHA"}))).await;
    assert_eq!(
        (s, &v["data"]["linkedRepo"]),
        (StatusCode::OK, &json!("alpha"))
    );
    let (s, v) = call(&app, "PATCH", &uri, Some(json!({"linkedRepo": null}))).await;
    assert_eq!(
        (s, &v["data"]["linkedRepo"]),
        (StatusCode::OK, &json!(null))
    );
    assert_eq!(v["data"]["title"], "login loop");

    let bad = [
        json!({"title": "  "}),
        json!({"title": "x".repeat(81)}),
        json!({"title": "two\nlines"}),
        json!({"name": "x"}),
    ];
    for body in bad {
        let (s, v) = call(&app, "PATCH", &uri, Some(body.clone())).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{body}: {v}");
    }
    let (s, _) = call(&app, "PATCH", &uri, Some(json!({"linkedRepo": "nope"}))).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = call(&app, "PATCH", "/api/conversations/abc", Some(json!({}))).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn end_marks_it_ended_and_a_message_continues_it() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (_state, app) = app_with(&env, &fake);
    let id = create(&app, "hello").await["conversation"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    idle(&app, &id).await;
    let (s, v) = call(&app, "POST", &format!("/api/conversations/{id}/end"), None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["data"]["ended"], true);
    assert_eq!(v["data"]["status"], "yourTurn");
    let (s, _) = message(&app, &id, "one more thing").await;
    assert_eq!(s, StatusCode::OK);
    let d = idle(&app, &id).await;
    assert_eq!(d["conversation"]["ended"], false);
    assert_eq!(fake.turns(), 2);
}

#[tokio::test]
async fn bodies_and_ids_are_checked() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (_state, app) = app_with(&env, &fake);
    let bad = [
        json!({"text": "   "}),
        json!({"text": "hi", "repo": "alpha"}),
        json!({"text": "hi", "attachments": ["nope"]}),
        json!({"text": "x".repeat(100_001)}),
    ];
    for body in bad {
        let (s, v) = call(&app, "POST", "/api/conversations", Some(body.clone())).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");
    }
    let (s, v) = call(&app, "GET", "/api/conversations", None).await;
    assert_eq!(
        (s, v["data"].as_array().unwrap().len()),
        (StatusCode::OK, 0)
    );
    for (method, uri) in [
        ("GET", "/api/conversations/abc"),
        ("POST", "/api/conversations/abc/stop"),
        ("POST", "/api/conversations/abc/end"),
    ] {
        let (s, _) = call(&app, method, uri, None).await;
        assert_eq!(s, StatusCode::NOT_FOUND, "{method} {uri}");
    }
    let (s, _) = message(&app, "abc", "hi").await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = message(&app, "../x", "hi").await;
    assert!(s == StatusCode::BAD_REQUEST || s == StatusCode::NOT_FOUND);
    assert_eq!(fake.turns(), 0);
}
