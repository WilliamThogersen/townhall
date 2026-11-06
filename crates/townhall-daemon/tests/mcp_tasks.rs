use serde_json::json;
use townhall_core::model::{ServerEvent, TaskState};

mod common;
mod mcp_support;
use common::{call, unique};
use mcp_support::{task, Mcp};

#[tokio::test]
async fn create_task_needs_a_repo_and_defaults_to_the_conversations() {
    let m = Mcp::new().await;
    m.add_repo("admin").await;
    let none = m.refused("create_task", task(&unique("a"))).await;
    assert!(none.contains("admin") && none.contains("shop"), "{none}");

    m.note_repo("shop").await;
    let name = unique("fix-login");
    let created = m.ok("create_task", task(&name)).await;
    assert_eq!(created["name"], name.as_str());
    assert_eq!(created["repo"], "shop");
    assert_eq!(created["baseBranch"], "main");
    assert_eq!(created["state"], "queued");
    assert_eq!(created["conversationId"], m.id.as_str());

    let patch = |c: townhall_core::model::Conversation| {
        Ok(townhall_core::model::Conversation {
            linked_repo: Some("admin".into()),
            ..c
        })
    };
    m.store().update_conversation(&m.id, patch).await.unwrap();
    let linked = m.ok("create_task", task(&unique("b"))).await;
    assert_eq!(linked["repo"], "admin");

    let explicit = json!({"repo": "SHOP", "baseBranch": "main", "after": [name]});
    let mut args = task(&unique("c"));
    args.as_object_mut()
        .unwrap()
        .extend(explicit.as_object().unwrap().clone());
    let picked = m.ok("create_task", args).await;
    assert_eq!(
        (&picked["repo"], &picked["after"]),
        (&json!("shop"), &json!([name]))
    );

    let mut bad_repo = task(&unique("d"));
    bad_repo["repo"] = json!("nope");
    let text = m.refused("create_task", bad_repo).await;
    assert!(
        text.contains("nope") && text.contains("admin, shop"),
        "{text}"
    );
    let mut bad_base = task(&unique("e"));
    bad_base["baseBranch"] = json!("no-such-branch");
    let text = m.refused("create_task", bad_base).await;
    assert!(text.contains("no-such-branch"), "{text}");
}

#[tokio::test]
async fn create_task_refuses_bad_input_with_a_reason() {
    let m = Mcp::new().await;
    m.note_repo("shop").await;
    let name = unique("dup");
    m.ok("create_task", task(&name)).await;
    let dup = m.refused("create_task", task(&name.to_uppercase())).await;
    assert!(dup.contains("already exists"), "{dup}");

    let mut ghost = task(&unique("g"));
    ghost["after"] = json!(["ghost-task"]);
    let text = m.refused("create_task", ghost).await;
    assert!(text.contains("ghost-task"), "{text}");

    let mut sneaky = task(&unique("s"));
    sneaky["permissionMode"] = json!("bypassPermissions");
    let text = m.refused("create_task", sneaky).await;
    assert!(text.contains("permissionMode"), "{text}");

    let cases = [
        json!({"name": "has space", "title": "t", "prompt": "p"}),
        json!({"name": "x".repeat(65), "title": "t", "prompt": "p"}),
        json!({"name": unique("t"), "title": "t".repeat(81), "prompt": "p"}),
        json!({"name": unique("t"), "title": "  ", "prompt": "p"}),
        json!({"name": unique("t"), "title": "t", "prompt": ""}),
        json!({"name": unique("t"), "title": "t", "prompt": "p", "after": vec![name.clone(); 21]}),
        json!({"title": "t", "prompt": "p"}),
    ];
    for args in cases {
        let text = m.refused("create_task", args.clone()).await;
        assert!(!text.is_empty(), "{args}");
    }
    assert_eq!(m.store().all_tasks().await.unwrap().len(), 1);
}

#[tokio::test]
async fn create_task_takes_attachments_of_this_conversation() {
    let m = Mcp::new().await;
    m.note_repo("shop").await;
    let file = m
        .store()
        .save_attachment(&m.id, "shot.png", "image/png", vec![1, 2, 3])
        .await
        .unwrap();
    let mut args = task(&unique("att"));
    args["attachments"] = json!([file.id, file.id]);
    let created = m.ok("create_task", args).await;
    assert_eq!(created["attachments"], json!([file.id]));
    let mut unknown = task(&unique("att"));
    unknown["attachments"] = json!(["nope00000000"]);
    let text = m.refused("create_task", unknown).await;
    assert!(text.contains("nope00000000"), "{text}");
}

#[tokio::test]
async fn one_tasks_created_entry_per_turn() {
    let m = Mcp::new().await;
    m.note_repo("shop").await;
    m.you("make two tasks").await;
    let (a, b, c) = (unique("a"), unique("b"), unique("c"));
    m.ok("create_task", task(&a)).await;
    let mut events = m.state.ctx().bus().subscribe();
    m.ok("create_task", task(&b)).await;
    let created = m.tasks_created().await;
    assert_eq!(created.len(), 1);
    assert_eq!(created[0].1, [a.clone(), b.clone()]);
    let mut resent = None;
    while let Ok(event) = events.try_recv() {
        if let ServerEvent::EntryAppended { entry, .. } = event {
            resent = Some(entry);
        }
    }
    let resent = resent.expect("the merged entry is sent again");
    assert_eq!(resent.id, created[0].0);

    m.you("and one more").await;
    m.ok("create_task", task(&c)).await;
    let created = m.tasks_created().await;
    assert_eq!(created.len(), 2);
    assert_eq!(created[1].1, [c]);
    assert!(created[1].0 > created[0].0);
    let tasks = m.store().tasks(&m.id).await.unwrap();
    assert!(tasks.iter().all(|t| t.state == TaskState::Queued));
}

#[tokio::test]
async fn create_task_refuses_a_name_an_agent_or_branch_has() {
    let m = Mcp::new().await;
    m.note_repo("shop").await;
    let branch_name = unique("branch");
    let branch = format!("townhall/{branch_name}");
    common::git(&m.env.startup, &["branch", &branch]).await;
    let text = m.refused("create_task", task(&branch_name)).await;
    assert!(text.contains(&branch) && text.contains("shop"), "{text}");

    let agent = unique("agent");
    common::start_shell(&m.env.workspace(&m.env.startup), &agent, "sleep 60").await;
    let text = m.refused("create_task", task(&agent)).await;
    call(&m.app, "DELETE", &format!("/api/agents/{agent}"), None).await;
    assert!(text.contains("an agent named"), "{text}");
    assert!(m.store().all_tasks().await.unwrap().is_empty());
}

#[tokio::test]
async fn create_task_refuses_a_name_an_agent_has_in_another_case() {
    let m = Mcp::new().await;
    m.note_repo("shop").await;
    m.add_repo("web").await;
    let zed = unique("zed");
    let web = m.env.root.path().join("web");
    common::start_shell(&m.env.workspace(&web), &zed, "sleep 60").await;
    let text = m.refused("create_task", task(&zed.to_uppercase())).await;
    call(
        &m.app,
        "DELETE",
        &format!("/api/agents/{zed}?repo=web"),
        None,
    )
    .await;
    let named = format!("an agent named `{zed}` already exists in repo `web`");
    assert!(text.contains(&named), "{text}");
    assert!(m.store().all_tasks().await.unwrap().is_empty());
}

#[tokio::test]
async fn create_task_takes_a_pr_target_that_is_an_existing_branch_and_never_a_sha() {
    let m = Mcp::new().await;
    m.note_repo("shop").await;
    common::git(&m.env.startup, &["branch", "reviewed"]).await;
    let head = common::git(&m.env.startup, &["rev-parse", "main"]).await;

    let plain = m.ok("create_task", task(&unique("plain"))).await;
    assert_eq!(plain["prBase"], json!(null));

    let mut aimed = task(&unique("ship"));
    aimed["baseBranch"] = json!("reviewed");
    aimed["prBase"] = json!(" main ");
    let created = m.ok("create_task", aimed).await;
    assert_eq!(created["baseBranch"], "reviewed");
    assert_eq!(created["prBase"], "main");
    let name = created["name"].as_str().unwrap();
    let stored = m.store().find_task(name).await.unwrap().unwrap();
    assert_eq!(stored.pr_base.as_deref(), Some("main"));
    assert_eq!(stored.pr_target(), "main");

    let refused = [
        ("no-such-branch", "does not exist"),
        ("   ", "must not be blank"),
        ("", "must not be blank"),
        (head.as_str(), "not a commit id"),
        ("-main", "not a plain branch name"),
        ("main^", "not a plain branch name"),
    ];
    for (bad, why) in refused {
        let mut args = task(&unique("bad"));
        args["prBase"] = json!(bad);
        let text = m.refused("create_task", args).await;
        assert!(text.contains(why), "`{bad}`: {text}");
    }
    assert_eq!(m.store().all_tasks().await.unwrap().len(), 2);
}

async fn second_commit(repo: &std::path::Path) {
    let args = [
        "-c",
        "user.name=t",
        "-c",
        "user.email=t@t",
        "commit",
        "--allow-empty",
        "-m",
        "second",
    ];
    common::git(repo, &args).await;
}

async fn refuses_pr_bases(m: &Mcp, bad: &[&str], why: &str) {
    for name in bad {
        let mut args = task(&unique("bad"));
        args["prBase"] = json!(name);
        let text = m.refused("create_task", args).await;
        assert!(text.contains(why), "`{name}`: {text}");
    }
}

#[tokio::test]
async fn a_pr_target_is_a_branch_name_and_never_revision_syntax() {
    let m = Mcp::new().await;
    m.note_repo("shop").await;
    let repo = &m.env.startup;
    second_commit(repo).await;
    common::git(repo, &["branch", "release/1.0"]).await;
    let head = common::git(repo, &["rev-parse", "main"]).await;
    assert_eq!(head.trim().len(), 40);

    let plain = [
        "main^",
        "main~1",
        "main@{1}",
        "main:README",
        "-x",
        "HEAD",
        "a b",
        "main.lock",
        "main..release/1.0",
    ];
    refuses_pr_bases(&m, &plain, "not a plain branch name").await;
    refuses_pr_bases(&m, &[head.trim()], "not a commit id").await;
    refuses_pr_bases(&m, &["nope"], "does not exist").await;
    for good in ["main", "release/1.0"] {
        let mut args = task(&unique("good"));
        args["prBase"] = json!(good);
        assert_eq!(m.ok("create_task", args).await["prBase"], good);
    }
}

#[tokio::test]
async fn with_an_origin_a_pr_target_must_be_on_origin() {
    let m = Mcp::new().await;
    m.note_repo("shop").await;
    let repo = &m.env.startup;
    second_commit(repo).await;
    common::git(repo, &["branch", "local-only"]).await;
    common::git(
        repo,
        &["remote", "add", "origin", "https://example.com/shop.git"],
    )
    .await;
    common::git(repo, &["update-ref", "refs/remotes/origin/shipped", "main"]).await;

    refuses_pr_bases(&m, &["local-only", "main"], "not on origin: push it first").await;
    refuses_pr_bases(&m, &["main^", "shipped^"], "not a plain branch name").await;
    let mut args = task(&unique("good"));
    args["prBase"] = json!("shipped");
    assert_eq!(m.ok("create_task", args).await["prBase"], "shipped");
}
