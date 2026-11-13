use serde_json::json;
use townhall_core::model::EntryBody;

mod common;
mod mcp_support;
use mcp_support::{task, Mcp};

const PIPE: &str = "name = \"quick\"\ndescription = \"build only\"\nwork = \"build\"\n\n[[step]]\nid = \"build\"\nagent = \"builder\"\n";

#[tokio::test]
async fn list_and_read_show_the_saved_pipelines() {
    let m = Mcp::new().await;
    let empty = m.ok("list_pipelines", json!({})).await;
    assert_eq!(empty["pipelines"], json!([]));
    m.store().save_workflow(PIPE).await.unwrap();
    let listed = m.ok("list_pipelines", json!({})).await;
    let rows = listed["pipelines"].as_array().unwrap();
    assert_eq!(rows.len(), 1, "{listed}");
    assert_eq!(rows[0]["name"], "quick");
    assert_eq!(rows[0]["description"], "build only");
    assert_eq!(rows[0]["steps"], json!(["build"]));
    let read = m.ok("read_pipeline", json!({"name": "QUICK"})).await;
    assert_eq!(read["name"], "quick");
    assert_eq!(read["source"], PIPE);
    let unknown = m.refused("read_pipeline", json!({"name": "ghost"})).await;
    assert!(
        unknown.contains("ghost") && unknown.contains("quick"),
        "{unknown}"
    );
}

#[tokio::test]
async fn an_invalid_pipeline_comes_back_as_its_errors() {
    let m = Mcp::new().await;
    let bad = PIPE.replace("agent = \"builder\"", "agent = \"ghost\"");
    let text = m.refused("save_pipeline", json!({"source": bad})).await;
    assert!(
        text.contains("error: workflow \"quick\"")
            && text.contains("\"ghost\" is not a saved agent"),
        "{text}"
    );
    assert!(m.store().workflow("quick").await.unwrap().is_none());
}

#[tokio::test]
async fn a_pipeline_asks_first_and_saves_once_confirmed() {
    let m = Mcp::new().await;
    let asked = m.ok("save_pipeline", json!({"source": PIPE})).await;
    assert_eq!(
        (&asked["status"], &asked["replaces"]),
        (&json!("asked"), &json!(false))
    );
    assert!(
        asked["next"].as_str().unwrap().contains("save_pipeline"),
        "{asked}"
    );
    assert!(m.store().workflow("quick").await.unwrap().is_none());
    let entries = m.store().entries(&m.id).await.unwrap();
    match &entries.last().unwrap().body {
        EntryBody::Townhall { text } => {
            assert!(
                text.contains("pipeline `quick`") && text.contains(PIPE),
                "{text}"
            )
        }
        other => panic!("expected a townhall entry, got {other:?}"),
    }

    let early = json!({"source": PIPE, "confirmed": true});
    let text = m.refused("save_pipeline", early.clone()).await;
    assert!(text.contains("has not answered"), "{text}");
    m.you("yes").await;
    let changed = json!({"source": PIPE.replace("build only", "x"), "confirmed": true});
    let text = m.refused("save_pipeline", changed).await;
    assert!(text.contains("differs"), "{text}");
    let done = m.ok("save_pipeline", early.clone()).await;
    assert_eq!(
        (&done["status"], &done["replaced"]),
        (&json!("saved"), &json!(false))
    );
    assert!(m.store().workflow("quick").await.unwrap().is_some());
    let again = m.refused("save_pipeline", early).await;
    assert!(again.contains("was not shown"), "{again}");

    let replace = PIPE.replace("build only", "changed");
    let asked = m.ok("save_pipeline", json!({"source": replace})).await;
    assert_eq!(asked["replaces"], true);
}

#[tokio::test]
async fn a_pipeline_and_an_agent_type_of_one_name_ask_separately() {
    let m = Mcp::new().await;
    let same = PIPE.replace("quick", "builder");
    m.ok("save_pipeline", json!({"source": same})).await;
    let kind = "name = \"builder\"\nadapter = \"claude\"\n";
    m.ok("save_agent_type", json!({"source": kind})).await;
    m.you("yes").await;
    let done = m
        .ok("save_pipeline", json!({"source": same, "confirmed": true}))
        .await;
    assert_eq!(done["status"], "saved");
}

#[tokio::test]
async fn the_session_may_use_the_pipeline_tools() {
    let allowed = townhall_core::mcp::allowed_tools();
    for tool in ["list_pipelines", "read_pipeline", "save_pipeline"] {
        assert!(
            allowed.contains(&format!("mcp__townhall__{tool}")),
            "{tool}"
        );
    }
}

const DEFAULT_PIPE: &str = "name = \"standard\"\nrepos = [\"shop\"]\ndefault = true\nwork = \"build\"\n\n[[step]]\nid = \"build\"\nagent = \"builder\"\n";

#[tokio::test]
async fn create_task_without_a_pipeline_and_without_a_default_makes_a_plain_task() {
    let m = Mcp::new().await;
    m.note_repo("shop").await;
    m.store().save_workflow(PIPE).await.unwrap();
    let made = m.ok("create_task", task("plain-one")).await;
    assert_eq!(made["name"], "plain-one");
    assert!(m.store().run("plain-one").await.unwrap().is_none());
}

#[tokio::test]
async fn create_task_uses_the_default_pipeline_and_an_empty_one_opts_out() {
    let m = Mcp::new().await;
    m.note_repo("shop").await;
    m.store().save_workflow(DEFAULT_PIPE).await.unwrap();

    let made = m.ok("create_task", task("via-default")).await;
    assert_eq!(made["name"], "via-default-build");
    let run = m.store().run("via-default").await.unwrap().unwrap();
    assert_eq!(run.workflow, "standard");

    let mut opted = task("opted-out");
    opted["pipeline"] = json!("");
    let plain = m.ok("create_task", opted).await;
    assert_eq!(plain["name"], "opted-out");
    assert!(m.store().run("opted-out").await.unwrap().is_none());

    let mut chained = task("chained");
    chained["after"] = json!(["opted-out"]);
    let after = m.ok("create_task", chained).await;
    assert_eq!(after["name"], "chained");
    assert!(m.store().run("chained").await.unwrap().is_none());
}

#[tokio::test]
async fn a_second_default_is_refused_through_save_pipeline() {
    let m = Mcp::new().await;
    m.store().save_workflow(DEFAULT_PIPE).await.unwrap();
    let other = DEFAULT_PIPE.replace("standard", "other");
    let text = m.refused("save_pipeline", json!({"source": other})).await;
    assert!(text.contains("already the default"), "{text}");
}

#[tokio::test]
async fn create_task_with_an_explicit_agent_makes_one_plain_task_even_with_a_default_pipeline() {
    let m = Mcp::new().await;
    m.note_repo("shop").await;
    m.store().save_workflow(DEFAULT_PIPE).await.unwrap();
    let coder = "name = \"coder\"\nadapter = \"claude\"\n";
    let planner = "name = \"planner\"\nadapter = \"claude\"\npermission_mode = \"plan\"\n";
    m.store().save_agent_type(coder).await.unwrap();
    m.store().save_agent_type(planner).await.unwrap();

    let mut explicit = task("fix-planner-prompt");
    explicit["agent"] = json!("coder");
    let made = m.ok("create_task", explicit).await;
    assert_eq!(made["name"], "fix-planner-prompt");
    assert_eq!(made["agent"], "coder");
    assert!(m.store().run("fix-planner-prompt").await.unwrap().is_none());
    let built = m
        .store()
        .find_task("fix-planner-prompt-build")
        .await
        .unwrap();
    assert!(built.is_none());
    let stored = m
        .store()
        .find_task("fix-planner-prompt")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.agent, "coder");
}
