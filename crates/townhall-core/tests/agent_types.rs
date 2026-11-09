use townhall_core::definitions::{AdapterKind, Effort, PermissionMode};
use townhall_core::model::CONVERSATION_AGENT_TYPE;
use townhall_core::store::Store;
use townhall_core::Error;

mod common;
use common::store_env;

const CODER: &str = "# reviews first\nname = \"coder\"\ndescription = \"writes code\"\nadapter = \"claude\"\nmodel = \"opus\"\npermission_mode = \"plan\"\n";
const RUNNER: &str = "name = \"runner\"\nadapter = \"shell\"\ncommand = \"cargo test\"\n";

#[tokio::test]
async fn a_saved_type_keeps_its_toml_and_reads_back_parsed() {
    let env = store_env();
    let (saved, replaced) = env.store.save_agent_type(CODER).await.unwrap();
    assert!(!replaced);
    assert_eq!(saved.source, CODER);
    assert_eq!(saved.def.name, "coder");
    assert_eq!(saved.def.model.as_deref(), Some("opus"));
    assert_eq!(saved.def.permission_mode, Some(PermissionMode::Plan));
    assert!(saved.updated_at.ends_with('Z'));

    let got = env.store.agent_type("CODER").await.unwrap().unwrap();
    assert_eq!(got, saved);
    assert!(env.store.agent_type("nope").await.unwrap().is_none());

    env.store.save_agent_type(RUNNER).await.unwrap();
    let names: Vec<String> = env
        .store
        .agent_types()
        .await
        .unwrap()
        .into_iter()
        .map(|t| t.def.name)
        .collect();
    assert!(names.contains(&"coder".to_string()) && names.contains(&"runner".to_string()));
    let runner = env.store.agent_type("runner").await.unwrap().unwrap();
    assert_eq!(runner.def.adapter, AdapterKind::Shell);
    let summary = runner.summary();
    assert_eq!(
        (
            summary.name.as_str(),
            summary.model,
            summary.permission_mode
        ),
        ("runner", None, None)
    );
}

#[tokio::test]
async fn effort_round_trips_through_save_and_read() {
    let env = store_env();
    let source = "name = \"calm\"\nadapter = \"claude\"\neffort = \"medium\"\n";
    let (saved, _) = env.store.save_agent_type(source).await.unwrap();
    assert_eq!(saved.def.effort, Some(Effort::Medium));
    let got = env.store.agent_type("calm").await.unwrap().unwrap();
    assert_eq!(got, saved);
    assert_eq!(got.summary().effort, Some(Effort::Medium));
    let plain = env.store.save_agent_type(RUNNER).await.unwrap().0;
    assert_eq!(plain.def.effort, None);
}

#[tokio::test]
async fn saving_the_same_name_replaces_ignoring_case() {
    let env = store_env();
    env.store.save_agent_type(CODER).await.unwrap();
    let again = CODER
        .replace("\"coder\"", "\"Coder\"")
        .replace("opus", "haiku");
    let (saved, replaced) = env.store.save_agent_type(&again).await.unwrap();
    assert!(replaced);
    assert_eq!(saved.def.name, "Coder");
    let got = env.store.agent_type("coder").await.unwrap().unwrap();
    assert_eq!(
        (got.def.name.as_str(), got.source.as_str()),
        ("Coder", again.as_str())
    );
    let all = env.store.agent_types().await.unwrap();
    assert_eq!(
        all.iter()
            .filter(|t| t.def.name.eq_ignore_ascii_case("coder"))
            .count(),
        1
    );
}

#[tokio::test]
async fn an_invalid_type_is_refused_with_every_error() {
    let env = store_env();
    let bad = "name = \"has space\"\nadapter = \"shell\"\n";
    let errors = Store::check_agent_type(bad).unwrap_err();
    assert_eq!(errors.len(), 2, "{errors:?}");
    let refused = env.store.save_agent_type(bad).await.unwrap_err();
    assert!(matches!(refused, Error::InvalidInput(_)), "{refused:?}");
    let skip = "name = \"x\"\nadapter = \"claude\"\npermission_mode = \"bypassPermissions\"\n";
    assert!(Store::check_agent_type(skip).is_err());
    let long = format!(
        "name = \"long\"\nadapter = \"claude\"\nprompt = \"{}\"\n",
        "a".repeat(Store::MAX_AGENT_SOURCE)
    );
    let errors = Store::check_agent_type(&long).unwrap_err();
    assert!(errors[0].message.contains("characters"), "{errors:?}");
    assert!(env.store.agent_type("x").await.unwrap().is_none());
}

#[tokio::test]
async fn delete_needs_a_known_type_and_keeps_the_last_one() {
    let env = store_env();
    let missing = env.store.delete_agent_type("ghost").await.unwrap_err();
    assert!(matches!(missing, Error::Missing { .. }), "{missing:?}");
    env.store.save_agent_type(CODER).await.unwrap();
    env.store.save_agent_type(RUNNER).await.unwrap();
    let all = env.store.agent_types().await.unwrap();
    let all: Vec<_> = all
        .into_iter()
        .filter(|t| t.def.name != CONVERSATION_AGENT_TYPE)
        .collect();
    for t in &all[..all.len() - 1] {
        assert_eq!(
            env.store
                .delete_agent_type(&t.def.name.to_uppercase())
                .await
                .unwrap(),
            t.def.name
        );
    }
    let last = &all[all.len() - 1].def.name;
    let kept = env.store.delete_agent_type(last).await.unwrap_err();
    assert!(matches!(kept, Error::Conflict(_)), "{kept:?}");
    assert!(env.store.agent_type(last).await.unwrap().is_some());
}

#[tokio::test]
async fn the_first_open_seeds_builder() {
    let env = store_env();
    let all = env.store.agent_types().await.unwrap();
    let names: Vec<&str> = all.iter().map(|t| t.def.name.as_str()).collect();
    assert_eq!(names, ["builder", "default"]);
    let builder = &all[0];
    assert_eq!(builder.def.description, "implements a task, tests first");
    assert_eq!(builder.def.adapter, AdapterKind::Claude);
    assert_eq!(builder.def.model.as_deref(), Some("sonnet"));
    assert_eq!(
        builder.def.permission_mode,
        Some(PermissionMode::AcceptEdits)
    );
    assert!(builder.def.tools.is_empty() && builder.def.command.is_none());
    let prompt = builder.def.prompt.as_deref().unwrap().to_lowercase();
    for rule in [
        "test first",
        "commit",
        "conventional",
        "stay inside the task",
        "ask",
    ] {
        assert!(prompt.contains(rule), "{rule}: {prompt}");
    }
    assert!(Store::check_agent_type(&builder.source).is_ok());
    assert!(builder.source.starts_with('#'));
    assert!(!builder.source.contains('\u{2014}') && !builder.source.contains('\u{2013}'));
}

#[tokio::test]
async fn a_deleted_builder_never_comes_back() {
    let dir = tempfile::TempDir::new().unwrap();
    let home = townhall_core::Home::at(dir.path().join(".townhall"));
    let open = || Store::open(&home, townhall_core::events::EventBus::new()).unwrap();
    let store = open();
    store.save_agent_type(CODER).await.unwrap();
    assert_eq!(store.delete_agent_type("builder").await.unwrap(), "builder");
    drop(store);
    let names: Vec<String> = open()
        .agent_types()
        .await
        .unwrap()
        .into_iter()
        .map(|t| t.def.name)
        .collect();
    assert_eq!(names, ["coder", "default"]);
}

#[tokio::test]
async fn the_first_open_seeds_the_conversation_default() {
    let env = store_env();
    let default = env.store.agent_type("default").await.unwrap().unwrap();
    assert_eq!(CONVERSATION_AGENT_TYPE, "default");
    assert_eq!(default.def.adapter, AdapterKind::Claude);
    assert_eq!(default.def.model, None);
    assert_eq!(default.def.permission_mode, Some(PermissionMode::Default));
    for tool in [
        "mcp__townhall__create_task",
        "mcp__townhall__list_agent_types",
        "Read",
        "Grep",
        "Glob",
    ] {
        assert!(default.def.tools.iter().any(|t| t == tool), "{tool}");
    }
    assert!(!default.def.tools.iter().any(|t| t == "Bash" || t == "Edit"));
    let prompt = default.def.prompt.as_deref().unwrap();
    assert!(prompt.starts_with("you are townhall, a planner"));
    assert!(prompt.contains("keep replies short."));
    assert!(!prompt.contains("repo for this turn"));
    assert!(Store::check_agent_type(&default.source).is_ok());
}

#[tokio::test]
async fn the_conversation_default_is_edited_never_deleted() {
    let env = store_env();
    env.store.save_agent_type(CODER).await.unwrap();
    let refused = env.store.delete_agent_type("DEFAULT").await.unwrap_err();
    assert!(matches!(refused, Error::Conflict(_)), "{refused:?}");
    assert!(env.store.agent_type("default").await.unwrap().is_some());
    let edited = "name = \"default\"\nadapter = \"claude\"\nmodel = \"opus\"\n";
    let (saved, replaced) = env.store.save_agent_type(edited).await.unwrap();
    assert!(replaced && saved.def.model.as_deref() == Some("opus"));
}

#[tokio::test]
async fn the_default_does_not_count_as_a_type_for_tasks() {
    let env = store_env();
    let refused = env.store.delete_agent_type("builder").await.unwrap_err();
    assert!(matches!(refused, Error::Conflict(_)), "{refused:?}");
    assert!(env.store.agent_type("builder").await.unwrap().is_some());
}
