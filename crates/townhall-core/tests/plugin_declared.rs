use serde_json::json;
use townhall_core::plugins::declared;
use townhall_core::plugins::prompts::Starter;

mod common;
use common::store_env;
mod plugin_support;
use plugin_support::{everything, install_everything, install_files, STARTERS, THEME};

#[tokio::test]
async fn no_plugin_means_nothing_declared() {
    let env = store_env();
    assert!(declared::enabled(&env.store).await.unwrap().is_empty());
    assert!(declared::brief_sections(&env.store, &["notes@acme".into()])
        .await
        .unwrap()
        .is_empty());
    assert!(declared::planner_notes(&env.store)
        .await
        .unwrap()
        .is_empty());
    let all = declared::contributions(&env.store).await.unwrap();
    assert!(all.starters.is_empty() && all.themes.is_empty() && all.settings.is_empty());
    assert!(all.theme.is_none());
}

#[tokio::test]
async fn an_enabled_plugins_declared_files_are_read_and_trimmed() {
    let env = store_env();
    install_everything(&env.store, "notes", "acme", true).await;
    let found = declared::enabled(&env.store).await.unwrap();
    assert_eq!(found.len(), 1);
    let notes = &found[0];
    assert_eq!(notes.id, "notes@acme");
    assert_eq!(notes.name, "notes");
    assert_eq!(
        notes.starters,
        [Starter {
            title: "tidy".into(),
            text: "tidy the readme".into()
        }]
    );
    assert_eq!(
        notes.brief.as_deref(),
        Some("always write a changelog line")
    );
    assert_eq!(notes.planner.as_deref(), Some("prefer small tasks"));
    assert_eq!(notes.fields.len(), 2);
    assert_eq!(
        notes.theme.as_deref(),
        Some(":root {\n  --th-accent: #ff00aa;\n}\n")
    );
}

#[tokio::test]
async fn a_disabled_plugin_declares_nothing() {
    let env = store_env();
    install_everything(&env.store, "notes", "acme", false).await;
    assert!(declared::enabled(&env.store).await.unwrap().is_empty());
    let all = declared::contributions(&env.store).await.unwrap();
    assert!(all.starters.is_empty() && all.themes.is_empty() && all.settings.is_empty());
}

#[tokio::test]
async fn a_contribution_that_fails_is_skipped_and_the_others_stay() {
    let env = store_env();
    install_files(
        &env.store,
        "notes",
        "acme",
        everything("notes"),
        &[
            ("prompts/starters.json", STARTERS),
            ("prompts/brief.md", "   "),
            ("prompts/planner.md", "prefer small tasks"),
            ("settings.schema.json", "{\"fields\": 3}"),
            ("theme.css", "@import \"https://evil.example/x.css\";"),
        ],
        true,
    )
    .await;
    let found = declared::enabled(&env.store).await.unwrap();
    let notes = &found[0];
    assert_eq!(notes.starters.len(), 1);
    assert_eq!(notes.brief, None);
    assert_eq!(notes.planner.as_deref(), Some("prefer small tasks"));
    assert!(notes.fields.is_empty());
    assert_eq!(notes.theme, None);
}

#[tokio::test]
async fn a_missing_file_or_one_leading_outside_the_plugin_is_skipped() {
    let env = store_env();
    let plugin = install_files(
        &env.store,
        "notes",
        "acme",
        everything("notes"),
        &[("prompts/starters.json", STARTERS), ("theme.css", THEME)],
        true,
    )
    .await;
    let root = env
        .store
        .home()
        .plugins_dir()
        .unwrap()
        .join("cache/acme/notes")
        .join(&plugin.sha);
    let outside = root.parent().unwrap().join("secret.md");
    std::fs::write(&outside, "leak").unwrap();
    std::os::unix::fs::symlink(&outside, root.join("prompts/brief.md")).unwrap();
    let found = declared::enabled(&env.store).await.unwrap();
    let notes = &found[0];
    assert_eq!(notes.brief, None);
    assert_eq!(notes.planner, None);
    assert_eq!(notes.starters.len(), 1);
    assert!(notes.theme.is_some());
}

#[tokio::test]
async fn the_brief_section_goes_only_to_the_plugins_an_agent_type_selects() {
    let env = store_env();
    install_everything(&env.store, "notes", "acme", true).await;
    install_files(
        &env.store,
        "other",
        "acme",
        json!({
            "name": "other", "version": "1.0.0", "apiVersion": 1,
            "permissions": { "prompts": ["brief"] },
            "contributes": { "prompts": { "brief": "b.md" } }
        }),
        &[("b.md", "other plugin text")],
        true,
    )
    .await;
    let selected = |names: &[&str]| {
        let names: Vec<String> = names.iter().map(|n| n.to_string()).collect();
        let store = env.store.clone();
        async move { declared::brief_sections(&store, &names).await.unwrap() }
    };
    assert!(selected(&[]).await.is_empty());
    assert!(selected(&["nope"]).await.is_empty());
    let by_name = selected(&["notes"]).await;
    assert_eq!(
        by_name,
        [(
            "notes@acme".to_string(),
            "always write a changelog line".to_string()
        )]
    );
    assert_eq!(selected(&["Notes@ACME"]).await, by_name);
    let both = selected(&["other", "notes"]).await;
    let ids: Vec<&str> = both.iter().map(|(id, _)| id.as_str()).collect();
    assert_eq!(ids, ["notes@acme", "other@acme"]);
}

#[tokio::test]
async fn the_planner_notes_come_from_every_enabled_plugin_that_has_one() {
    let env = store_env();
    install_everything(&env.store, "notes", "acme", true).await;
    install_everything(&env.store, "off", "acme", false).await;
    assert_eq!(
        declared::planner_notes(&env.store).await.unwrap(),
        [("notes@acme".to_string(), "prefer small tasks".to_string())]
    );
}

#[tokio::test]
async fn settings_groups_show_defaults_stored_values_and_hide_secrets() {
    let env = store_env();
    install_everything(&env.store, "notes", "acme", true).await;
    let group = &declared::contributions(&env.store).await.unwrap().settings[0];
    assert_eq!(group.plugin, "notes@acme");
    assert_eq!(group.name, "notes");
    assert_eq!(group.fields[0].value, json!(3));
    assert!(!group.fields[0].set);

    env.store
        .set_plugin_setting("notes@acme", "retries", json!(7))
        .await
        .unwrap();
    env.store
        .set_plugin_setting("notes@acme", "token", json!("hunter2"))
        .await
        .unwrap();
    let group = &declared::contributions(&env.store).await.unwrap().settings[0];
    assert_eq!(group.fields[0].value, json!(7));
    assert!(group.fields[0].set);
    assert_eq!(group.fields[1].value, json!(null));
    assert!(group.fields[1].set);
    let wire = serde_json::to_string(&declared::contributions(&env.store).await.unwrap()).unwrap();
    assert!(!wire.contains("hunter2"), "{wire}");
}

#[tokio::test]
async fn a_stored_value_that_no_longer_fits_the_schema_falls_back_to_the_default() {
    let env = store_env();
    install_everything(&env.store, "notes", "acme", true).await;
    env.store
        .set_plugin_setting("notes@acme", "retries", json!(99))
        .await
        .unwrap();
    let group = &declared::contributions(&env.store).await.unwrap().settings[0];
    assert_eq!(group.fields[0].value, json!(3));
    assert!(!group.fields[0].set);
}

#[tokio::test]
async fn choosing_a_theme_needs_an_enabled_plugin_that_ships_one() {
    let env = store_env();
    let store = &env.store;
    install_everything(store, "notes", "acme", true).await;
    install_everything(store, "off", "acme", false).await;
    install_files(
        store,
        "plain",
        "acme",
        json!({"name": "plain", "version": "1.0.0", "apiVersion": 1}),
        &[],
        true,
    )
    .await;

    let all = declared::choose_theme(store, Some("NOTES@acme"))
        .await
        .unwrap();
    assert_eq!(all.theme.unwrap().plugin, "notes@acme");
    assert_eq!(
        store.plugin_theme().await.unwrap().as_deref(),
        Some("notes@acme")
    );

    for (id, kind) in [
        ("ghost@acme", "missing"),
        ("off@acme", "conflict"),
        ("plain@acme", "conflict"),
    ] {
        let err = declared::choose_theme(store, Some(id)).await.unwrap_err();
        let got = match err {
            townhall_core::Error::Missing { .. } => "missing",
            townhall_core::Error::Conflict(_) => "conflict",
            other => panic!("{id}: {other}"),
        };
        assert_eq!(got, kind, "{id}");
    }
    assert_eq!(
        store.plugin_theme().await.unwrap().as_deref(),
        Some("notes@acme")
    );

    let all = declared::choose_theme(store, None).await.unwrap();
    assert!(all.theme.is_none());
    assert_eq!(store.plugin_theme().await.unwrap(), None);
}

#[tokio::test]
async fn a_setting_is_saved_only_when_it_fits_its_field() {
    let env = store_env();
    let store = &env.store;
    install_everything(store, "notes", "acme", true).await;
    install_everything(store, "off", "acme", false).await;

    let all = declared::set_setting(store, "Notes@ACME", "retries", json!(5))
        .await
        .unwrap();
    assert_eq!(all.settings[0].fields[0].value, json!(5));
    assert_eq!(
        store.plugin_settings("notes@acme").await.unwrap()["retries"],
        json!(5)
    );

    for (key, value) in [
        ("retries", json!(10)),
        ("retries", json!("x")),
        ("token", json!(3)),
    ] {
        let err = declared::set_setting(store, "notes@acme", key, value)
            .await
            .unwrap_err();
        assert!(
            matches!(err, townhall_core::Error::InvalidInput(_)),
            "{key}: {err}"
        );
    }
    let err = declared::set_setting(store, "notes@acme", "nope", json!(1))
        .await
        .unwrap_err();
    assert!(matches!(err, townhall_core::Error::Missing { .. }), "{err}");
    let err = declared::set_setting(store, "off@acme", "retries", json!(2))
        .await
        .unwrap_err();
    assert!(matches!(err, townhall_core::Error::Missing { .. }), "{err}");
    let err = declared::set_setting(store, "ghost@acme", "retries", json!(2))
        .await
        .unwrap_err();
    assert!(matches!(err, townhall_core::Error::Missing { .. }), "{err}");
    assert_eq!(store.plugin_settings("notes@acme").await.unwrap().len(), 1);
    assert!(store.plugin_settings("off@acme").await.unwrap().is_empty());

    let all = declared::reset_setting(store, "notes@acme", "retries")
        .await
        .unwrap();
    assert_eq!(all.settings[0].fields[0].value, json!(3));
    assert!(store
        .plugin_settings("notes@acme")
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn uninstalling_keeps_settings_unless_purged_and_drops_its_own_theme() {
    use townhall_core::plugins::install::PluginRegistry;
    let env = store_env();
    let store = &env.store;
    let registry = PluginRegistry::new(store.clone());
    install_everything(store, "notes", "acme", true).await;
    install_everything(store, "other", "acme", true).await;
    declared::set_setting(store, "notes@acme", "retries", json!(5))
        .await
        .unwrap();
    declared::set_setting(store, "other@acme", "retries", json!(6))
        .await
        .unwrap();
    declared::choose_theme(store, Some("notes@acme"))
        .await
        .unwrap();

    registry.uninstall("notes@acme", false).await.unwrap();
    assert_eq!(store.plugin_settings("notes@acme").await.unwrap().len(), 1);
    assert_eq!(store.plugin_theme().await.unwrap(), None);

    declared::choose_theme(store, Some("other@acme"))
        .await
        .unwrap();
    registry.uninstall("other@acme", true).await.unwrap();
    assert!(store
        .plugin_settings("other@acme")
        .await
        .unwrap()
        .is_empty());
    assert_eq!(store.plugin_theme().await.unwrap(), None);
    assert_eq!(store.plugin_settings("notes@acme").await.unwrap().len(), 1);
}

#[tokio::test]
async fn the_chosen_theme_applies_only_while_its_plugin_is_enabled() {
    let env = store_env();
    let plugin = install_everything(&env.store, "notes", "acme", true).await;
    let all = declared::contributions(&env.store).await.unwrap();
    assert_eq!(all.themes.len(), 1);
    assert_eq!(all.themes[0].plugin, "notes@acme");
    assert!(all.theme.is_none());

    env.store
        .set_plugin_theme(Some("notes@acme"))
        .await
        .unwrap();
    let all = declared::contributions(&env.store).await.unwrap();
    let active = all.theme.unwrap();
    assert_eq!(active.plugin, "notes@acme");
    assert!(active.css.contains("--th-accent: #ff00aa;"));

    env.store
        .update_plugin(townhall_core::store::Plugin {
            enabled: false,
            ..plugin
        })
        .await
        .unwrap();
    let all = declared::contributions(&env.store).await.unwrap();
    assert!(all.theme.is_none() && all.themes.is_empty());
}
