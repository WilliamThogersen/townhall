use townhall_core::model::{
    ArtifactKind, AuthorRole, NewArtifact, MAX_ARTIFACTS_PER_TASK, MAX_ARTIFACT_BYTES,
};
use townhall_core::Error;

mod common;
use common::{new_task, store_env};

const PNG: &[u8] = b"\x89PNG\r\n\x1a\nrest";

fn note(name: &str, text: &str) -> NewArtifact {
    NewArtifact {
        name: name.into(),
        kind: ArtifactKind::Note,
        author_role: AuthorRole::Coder,
        caption: None,
        bytes: text.as_bytes().to_vec(),
    }
}

async fn with_task(env: &common::StoreEnv, name: &str) -> String {
    let c = env.store.create_conversation().await.unwrap();
    env.store.create_task(&c.id, new_task(name)).await.unwrap();
    c.id
}

#[tokio::test]
async fn an_artifact_is_stored_under_its_conversation_and_task() {
    let env = store_env();
    let cid = with_task(&env, "fix-login").await;
    let mut new = note("proof.md", "it works");
    new.caption = Some("  the login now redirects  ".into());
    new.author_role = AuthorRole::Tester;
    let saved = env.store.add_artifact("fix-login", new).await.unwrap();
    assert_eq!(
        (saved.conversation_id.as_str(), saved.task.as_str()),
        (cid.as_str(), "fix-login")
    );
    assert_eq!(saved.name, "proof.md");
    assert_eq!(saved.mime, "text/markdown; charset=utf-8");
    assert_eq!(saved.size, 8);
    assert_eq!(saved.author_role, AuthorRole::Tester);
    assert_eq!(saved.caption.as_deref(), Some("the login now redirects"));
    assert_eq!(
        env.store.artifacts("fix-login").await.unwrap(),
        vec![saved.clone()]
    );
    let (found, path) = env.store.artifact("fix-login", &saved.id).await.unwrap();
    assert_eq!(found, saved);
    let expected = env
        .dir
        .path()
        .join(".townhall/artifacts")
        .join(&cid)
        .join("fix-login")
        .join(format!("{}-proof.md", saved.id));
    assert_eq!(path, expected);
    assert_eq!(std::fs::read_to_string(path).unwrap(), "it works");
}

#[tokio::test]
async fn names_cannot_leave_the_task_folder() {
    let env = store_env();
    with_task(&env, "fix-login").await;
    let saved = env
        .store
        .add_artifact("fix-login", note("../../../etc/passwd", "x"))
        .await
        .unwrap();
    assert_eq!(saved.name, "passwd");
    let (_, path) = env.store.artifact("fix-login", &saved.id).await.unwrap();
    assert!(path.starts_with(env.dir.path().join(".townhall/artifacts")));
    let bad = env.store.artifact("fix-login", "../x").await.unwrap_err();
    assert!(matches!(bad, Error::InvalidInput(_)), "{bad:?}");
}

#[tokio::test]
async fn a_screenshot_must_be_a_real_image_and_text_kinds_must_be_text() {
    let env = store_env();
    with_task(&env, "fix-login").await;
    let mut shot = note("shot", "");
    shot.kind = ArtifactKind::Screenshot;
    shot.bytes = PNG.to_vec();
    let saved = env
        .store
        .add_artifact("fix-login", shot.clone())
        .await
        .unwrap();
    assert_eq!(
        (saved.mime.as_str(), saved.name.as_str()),
        ("image/png", "shot.png")
    );
    shot.bytes = b"<svg onload=alert(1)>".to_vec();
    let fake = env.store.add_artifact("fix-login", shot).await.unwrap_err();
    assert!(
        matches!(&fake, Error::InvalidInput(m) if m.contains("image")),
        "{fake:?}"
    );
    let mut log = note("run.log", "");
    log.kind = ArtifactKind::Log;
    log.bytes = PNG.to_vec();
    let binary = env.store.add_artifact("fix-login", log).await.unwrap_err();
    assert!(
        matches!(&binary, Error::InvalidInput(m) if m.contains("text")),
        "{binary:?}"
    );
}

#[tokio::test]
async fn text_is_never_stored_as_html() {
    let env = store_env();
    with_task(&env, "fix-login").await;
    let saved = env
        .store
        .add_artifact("fix-login", note("page.html", "<script>alert(1)</script>"))
        .await
        .unwrap();
    assert_eq!(saved.mime, "text/markdown; charset=utf-8");
    let mut log = note("page.html", "<b>x</b>");
    log.kind = ArtifactKind::Log;
    let saved = env.store.add_artifact("fix-login", log).await.unwrap();
    assert_eq!(saved.mime, "text/plain; charset=utf-8");
}

#[tokio::test]
async fn the_size_and_count_caps_hold() {
    let env = store_env();
    with_task(&env, "fix-login").await;
    let mut big = note("big.log", "");
    big.bytes = vec![b'a'; usize::try_from(MAX_ARTIFACT_BYTES).unwrap() + 1];
    let err = env.store.add_artifact("fix-login", big).await.unwrap_err();
    assert!(matches!(err, Error::TooLarge(_)), "{err:?}");
    let empty = env
        .store
        .add_artifact("fix-login", note("e.md", ""))
        .await
        .unwrap_err();
    assert!(matches!(empty, Error::InvalidInput(_)), "{empty:?}");
    for i in 0..MAX_ARTIFACTS_PER_TASK {
        env.store
            .add_artifact("fix-login", note(&format!("n{i}.md"), "x"))
            .await
            .unwrap();
    }
    let full = env
        .store
        .add_artifact("fix-login", note("late.md", "x"))
        .await
        .unwrap_err();
    assert!(
        matches!(&full, Error::Conflict(m) if m.contains("50")),
        "{full:?}"
    );
    assert_eq!(
        env.store.artifacts("fix-login").await.unwrap().len(),
        MAX_ARTIFACTS_PER_TASK
    );
}

#[tokio::test]
async fn the_same_name_twice_keeps_both() {
    let env = store_env();
    with_task(&env, "fix-login").await;
    let a = env
        .store
        .add_artifact("fix-login", note("a.md", "one"))
        .await
        .unwrap();
    let b = env
        .store
        .add_artifact("fix-login", note("a.md", "two"))
        .await
        .unwrap();
    assert_ne!(a.id, b.id);
    let (_, first) = env.store.artifact("fix-login", &a.id).await.unwrap();
    assert_eq!(std::fs::read_to_string(first).unwrap(), "one");
}

#[tokio::test]
async fn an_unknown_task_has_no_artifacts() {
    let env = store_env();
    let err = env.store.artifacts("ghost").await.unwrap_err();
    assert!(matches!(err, Error::Missing { .. }), "{err:?}");
    let err = env
        .store
        .add_artifact("ghost", note("a.md", "x"))
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Missing { .. }), "{err:?}");
}
