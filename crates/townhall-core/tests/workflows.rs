use townhall_core::Error;

mod common;
use common::store_env;

const PLAN: &str = "name = \"plan-build\"\ndescription = \"plan then build\"\nwork = \"build\"\n\n[[step]]\nid = \"plan\"\nagent = \"builder\"\n\n[[step]]\nid = \"build\"\nagent = \"builder\"\nafter = [\"plan\"]\n";

#[tokio::test]
async fn a_saved_pipeline_keeps_its_toml_and_reads_back_parsed() {
    let env = store_env();
    let (saved, replaced) = env.store.save_workflow(PLAN).await.unwrap();
    assert!(!replaced);
    assert_eq!(saved.source, PLAN);
    assert_eq!(saved.def.work.as_deref(), Some("build"));
    assert_eq!(saved.def.steps.len(), 2);

    let got = env.store.workflow("PLAN-build").await.unwrap().unwrap();
    assert_eq!(got, saved);
    assert!(env.store.workflow("nope").await.unwrap().is_none());
    assert_eq!(env.store.workflows().await.unwrap(), [saved]);
}

#[tokio::test]
async fn saving_again_replaces_and_reports_it() {
    let env = store_env();
    env.store.save_workflow(PLAN).await.unwrap();
    let edited = PLAN.replace("plan then build", "changed");
    let (saved, replaced) = env.store.save_workflow(&edited).await.unwrap();
    assert!(replaced);
    assert_eq!(saved.def.description, "changed");
    assert_eq!(env.store.workflows().await.unwrap().len(), 1);
}

#[tokio::test]
async fn a_pipeline_naming_an_unknown_agent_is_refused() {
    let env = store_env();
    let bad = PLAN.replace("id = \"plan\"\nagent = \"builder\"", "id = \"plan\"\nagent = \"ghost\"");
    let refused = env.store.save_workflow(&bad).await.unwrap_err();
    assert!(matches!(refused, Error::InvalidInput(_)), "{refused:?}");
    assert!(
        refused.to_string().contains("\"ghost\" is not a saved agent"),
        "{refused}"
    );
    assert!(env.store.workflows().await.unwrap().is_empty());
}

#[tokio::test]
async fn checking_returns_every_field_error() {
    let env = store_env();
    let bad = PLAN.replace("work = \"build\"", "work = \"nowhere\"");
    let errors = env.store.check_pipeline(&bad).await.unwrap_err();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].field.as_deref(), Some("work"));
}

#[tokio::test]
async fn delete_removes_and_a_second_delete_is_missing() {
    let env = store_env();
    env.store.save_workflow(PLAN).await.unwrap();
    assert_eq!(env.store.delete_workflow("PLAN-BUILD").await.unwrap(), "plan-build");
    assert!(env.store.workflow("plan-build").await.unwrap().is_none());
    let again = env.store.delete_workflow("plan-build").await.unwrap_err();
    assert!(matches!(again, Error::Missing { .. }), "{again:?}");
}

#[tokio::test]
async fn run_steps_are_unique_per_run_step_and_round() {
    let env = store_env();
    let convo = env.store.create_conversation().await.unwrap().id;
    let duplicate = env
        .store
        .call(move |c| {
            c.execute(
                "insert into runs (id, workflow, snapshot, conversation_id, repo, base_branch, created_at)
                 values ('r1', 'w', 's', ?1, 'alpha', 'main', 'x')",
                [&convo],
            )?;
            let add = "insert into run_steps (run_id, step_id, round, state) values ('r1', 'plan', ?1, 'queued')";
            c.execute(add, [1])?;
            c.execute(add, [2])?;
            Ok(c.execute(add, [1]).is_err())
        })
        .await
        .unwrap();
    assert!(duplicate);
}
