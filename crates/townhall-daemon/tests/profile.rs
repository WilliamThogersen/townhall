use axum::http::StatusCode;
use serde_json::json;

mod common;
mod runner_support;
use common::{call, Env};
use runner_support::{app_with, create, idle, message, set_setting, values_of, Fake};

const TEXT: &str = "# how i work\nsmall steps, dry tone";

fn prompt(argv: &[String]) -> String {
    values_of(argv, "--append-system-prompt").join(" ")
}

#[tokio::test]
async fn the_profile_is_read_put_and_deleted() {
    let env = Env::new("alpha").await;
    let (_state, app) = env.start();
    let (s, v) = call(&app, "GET", "/api/profile", None).await;
    assert_eq!(
        (s, &v["data"]),
        (StatusCode::OK, &json!({"text": ""})),
        "{v}"
    );
    let put = Some(json!({"text": TEXT}));
    let (s, v) = call(&app, "PUT", "/api/profile", put).await;
    assert_eq!(
        (s, &v["data"]),
        (StatusCode::OK, &json!({"text": TEXT})),
        "{v}"
    );
    let (_, v) = call(&app, "GET", "/api/profile", None).await;
    assert_eq!(v["data"]["text"], TEXT);
    let put = Some(json!({"text": "second"}));
    let (_, v) = call(&app, "PUT", "/api/profile", put).await;
    assert_eq!(v["data"]["text"], "second", "a put replaces");
    let (s, v) = call(&app, "DELETE", "/api/profile", None).await;
    assert_eq!(
        (s, &v["data"]),
        (StatusCode::OK, &json!({"text": ""})),
        "{v}"
    );
    let (_, v) = call(&app, "GET", "/api/profile", None).await;
    assert_eq!(v["data"]["text"], "");
}

#[tokio::test]
async fn a_bad_profile_put_is_a_400() {
    let env = Env::new("alpha").await;
    let (_state, app) = env.start();
    let too_long = "x".repeat(20_001);
    let bad = [
        json!({"text": too_long}),
        json!({"text": 3}),
        json!({}),
        json!({"text": "a", "x": 1}),
    ];
    for body in bad {
        let (s, v) = call(&app, "PUT", "/api/profile", Some(body.clone())).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{body}: {v}");
        assert_eq!(v["success"], false);
    }
    let (_, v) = call(&app, "GET", "/api/profile", None).await;
    assert_eq!(v["data"]["text"], "");
}

#[tokio::test]
async fn learn_about_me_is_a_setting_that_starts_off() {
    let env = Env::new("alpha").await;
    let (_state, app) = env.start();
    let (_, v) = call(&app, "GET", "/api/settings", None).await;
    assert_eq!(v["data"]["learnAboutMe"], false);
    let put = Some(json!({"value": true}));
    let (s, v) = call(&app, "PUT", "/api/settings/learnAboutMe", put).await;
    assert_eq!(
        (s, &v["data"]["learnAboutMe"]),
        (StatusCode::OK, &json!(true))
    );
    let put = Some(json!({"value": "yes"}));
    let (s, _) = call(&app, "PUT", "/api/settings/learnAboutMe", put).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn the_profile_reaches_the_prompt_only_while_the_flag_is_on() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (_state, app) = app_with(&env, &fake);
    call(&app, "PUT", "/api/profile", Some(json!({"text": TEXT}))).await;

    let created = create(&app, "hello").await;
    let id = created["conversation"]["id"].as_str().unwrap().to_string();
    idle(&app, &id).await;
    assert!(!prompt(&fake.argv(1)).contains("dry tone"), "flag off");

    set_setting(&app, "learnAboutMe", json!(true)).await;
    let (s, v) = message(&app, &id, "again").await;
    assert_eq!(s, StatusCode::OK, "{v}");
    idle(&app, &id).await;
    assert!(prompt(&fake.argv(2)).contains("dry tone"), "flag on");

    call(&app, "DELETE", "/api/profile", None).await;
    message(&app, &id, "third").await;
    idle(&app, &id).await;
    assert!(
        !prompt(&fake.argv(3)).contains("about the user"),
        "empty profile"
    );

    call(&app, "PUT", "/api/profile", Some(json!({"text": TEXT}))).await;
    set_setting(&app, "learnAboutMe", json!(false)).await;
    message(&app, &id, "fourth").await;
    idle(&app, &id).await;
    assert!(
        !prompt(&fake.argv(4)).contains("dry tone"),
        "flag off again"
    );
}
