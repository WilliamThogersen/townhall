use townhall_core::model::{
    AttachmentKind, Checks, NewTask, Pr, Task, TaskState, MAX_ATTACHMENT_BYTES,
};
use townhall_core::Error;

mod common;
use common::{new_task, store_env, you};

#[tokio::test]
async fn task_names_are_unique_across_conversations_ignoring_case() {
    let env = store_env();
    let (a, b) = (
        env.store.create_conversation().await.unwrap(),
        env.store.create_conversation().await.unwrap(),
    );
    let t = env
        .store
        .create_task(&a.id, new_task("fix-login"))
        .await
        .unwrap();
    assert_eq!(
        (t.state, t.conversation_id.as_str()),
        (TaskState::Queued, a.id.as_str())
    );
    assert_eq!(t.branch(), "townhall/fix-login");
    let dup = env
        .store
        .create_task(&b.id, new_task("Fix-Login"))
        .await
        .unwrap_err();
    assert!(matches!(dup, Error::Conflict(_)), "{dup:?}");
    let bad = env
        .store
        .create_task(&b.id, new_task("no spaces"))
        .await
        .unwrap_err();
    assert!(matches!(bad, Error::InvalidName(_)), "{bad:?}");
    assert_eq!(env.store.all_tasks().await.unwrap(), vec![t.clone()]);
    assert_eq!(env.store.find_task("fix-login").await.unwrap(), Some(t));
    assert!(env.store.find_task("nope").await.unwrap().is_none());
    assert_eq!(
        env.store.conversation(&a.id).await.unwrap().tasks,
        vec!["fix-login"]
    );
}

#[tokio::test]
async fn a_tasks_check_and_verify_state_are_stored() {
    use townhall_core::model::{Done, Verify, VerifyPhase};
    let env = store_env();
    let c = env.store.create_conversation().await.unwrap();
    let new = NewTask {
        done: Done {
            done_when: Some("all tests pass".into()),
            check: Some("npm test".into()),
            max_retries: 4,
            verify: Some(Verify {
                phase: VerifyPhase::Passed,
                retries: 9,
                output: None,
            }),
        },
        ..new_task("checked")
    };
    let made = env.store.create_task(&c.id, new).await.unwrap();
    assert_eq!(made.done.done_when.as_deref(), Some("all tests pass"));
    assert_eq!(made.done.check.as_deref(), Some("npm test"));
    assert_eq!(made.done.max_retries, 4);
    assert_eq!(made.done.verify, None, "a new task has not been verified");
    let verify = Verify {
        phase: VerifyPhase::Failed,
        retries: 2,
        output: Some("1 failed".into()),
    };
    let set = verify.clone();
    let change = move |t: Task| {
        Ok(Task {
            done: Done {
                verify: Some(set),
                ..t.done
            },
            ..t
        })
    };
    env.store.update_task("checked", change).await.unwrap();
    let read = env.store.find_task("checked").await.unwrap().unwrap();
    assert_eq!(read.done.verify, Some(verify));
    assert_eq!(read.done.check.as_deref(), Some("npm test"));
    let plain = env
        .store
        .create_task(&c.id, new_task("plain"))
        .await
        .unwrap();
    assert_eq!(plain.done, Done::default());
}

#[tokio::test]
async fn after_is_deduped_and_attachments_are_kept() {
    let env = store_env();
    let c = env.store.create_conversation().await.unwrap();
    env.store
        .create_task(&c.id, new_task("first"))
        .await
        .unwrap();
    let waits = NewTask {
        after: vec!["first".into(), "FIRST".into()],
        ..new_task("second")
    };
    assert_eq!(
        env.store.create_task(&c.id, waits).await.unwrap().after,
        vec!["first"]
    );
    let file = env
        .store
        .save_attachment(&c.id, "notes.md", "text/markdown", b"# hi".to_vec())
        .await
        .unwrap();
    let with_file = NewTask {
        attachments: vec![file.id.clone()],
        ..new_task("third")
    };
    assert_eq!(
        env.store
            .create_task(&c.id, with_file)
            .await
            .unwrap()
            .attachments,
        vec![file.id]
    );
    assert_eq!(env.store.tasks(&c.id).await.unwrap().len(), 3);
}

#[tokio::test]
async fn after_and_attachments_must_exist() {
    let env = store_env();
    let c = env.store.create_conversation().await.unwrap();
    for bad in [
        NewTask {
            after: vec!["ghost".into()],
            ..new_task("x1")
        },
        NewTask {
            after: vec!["x2".into()],
            ..new_task("x2")
        },
        NewTask {
            attachments: vec!["nope".into()],
            ..new_task("x3")
        },
        NewTask {
            title: "  ".into(),
            ..new_task("x4")
        },
    ] {
        let err = env.store.create_task(&c.id, bad).await.unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)), "{err:?}");
    }
    assert!(env.store.tasks(&c.id).await.unwrap().is_empty());
}

#[tokio::test]
async fn update_task_keeps_identity_and_stores_the_pr() {
    let env = store_env();
    let c = env.store.create_conversation().await.unwrap();
    let t = env.store.create_task(&c.id, new_task("t1")).await.unwrap();
    let pr = Pr {
        number: 41,
        url: Some("https://x/41".into()),
        checks: Checks::Failed,
    };
    let pr_copy = pr.clone();
    let next = env
        .store
        .update_task("t1", move |t| {
            Ok(Task {
                name: "renamed".into(),
                state: TaskState::InCi,
                pr: Some(pr_copy),
                ..t
            })
        })
        .await
        .unwrap();
    assert_eq!((next.name.as_str(), next.state), ("t1", TaskState::InCi));
    assert_eq!(
        (next.created_at.as_str(), next.pr.as_ref()),
        (t.created_at.as_str(), Some(&pr))
    );
    assert_eq!(env.store.tasks(&c.id).await.unwrap(), vec![next]);
    let missing = env.store.update_task("ghost", Ok).await.unwrap_err();
    assert!(
        matches!(missing, Error::Missing { what: "task", .. }),
        "{missing:?}"
    );
}

#[tokio::test]
async fn attachments_are_files_with_a_record() {
    let env = store_env();
    let c = env.store.create_conversation().await.unwrap();
    let img = env
        .store
        .save_attachment(&c.id, "../../shot.png", "image/png", b"png".to_vec())
        .await
        .unwrap();
    assert_eq!(
        (img.name.as_str(), img.kind, img.size),
        ("shot.png", AttachmentKind::Image, 3)
    );
    let (found, path) = env.store.attachment(&c.id, &img.id).await.unwrap();
    assert_eq!(found, img);
    let expected = env
        .dir
        .path()
        .join(".townhall/conversations")
        .join(&c.id)
        .join("files")
        .join(&img.id)
        .join("shot.png");
    assert_eq!(path, expected);
    assert_eq!(std::fs::read(&path).unwrap(), b"png");
    let doc = env
        .store
        .save_attachment(&c.id, "", "", b"x".to_vec())
        .await
        .unwrap();
    assert_eq!(
        (doc.name.as_str(), doc.mime.as_str()),
        ("file", "application/octet-stream")
    );
    assert_eq!(env.store.attachments(&c.id).await.unwrap(), vec![img, doc]);
}

#[tokio::test]
async fn too_big_unknown_and_orphan_attachments_are_refused() {
    let env = store_env();
    let c = env.store.create_conversation().await.unwrap();
    let big = vec![0u8; MAX_ATTACHMENT_BYTES as usize + 1];
    let err = env
        .store
        .save_attachment(&c.id, "big.bin", "", big)
        .await
        .unwrap_err();
    assert!(matches!(err, Error::TooLarge(_)), "{err:?}");
    assert!(matches!(
        env.store.attachment(&c.id, "nope").await,
        Err(Error::Missing { .. })
    ));
    assert!(env
        .store
        .save_attachment("ghost", "a", "", vec![])
        .await
        .is_err());
}

#[tokio::test]
async fn detail_has_everything() {
    let env = store_env();
    let c = env.store.create_conversation().await.unwrap();
    env.store.append_entry(&c.id, you("hi")).await.unwrap();
    env.store.create_task(&c.id, new_task("t1")).await.unwrap();
    let d = env.store.detail(&c.id).await.unwrap();
    assert_eq!(d.conversation.tasks, vec!["t1".to_string()]);
    assert_eq!(
        (d.entries.len(), d.tasks.len(), d.attachments.len()),
        (1, 1, 0)
    );
}

#[tokio::test]
async fn a_block_reason_is_stored_and_cleared() {
    let env = store_env();
    let c = env.store.create_conversation().await.unwrap();
    let t = env
        .store
        .create_task(&c.id, new_task("ship"))
        .await
        .unwrap();
    assert_eq!(t.blocked, None);
    let why = "reviewer reported REJECT";
    let set = move |t: Task| {
        Ok(Task {
            blocked: Some(why.into()),
            ..t
        })
    };
    env.store.update_task("ship", set).await.unwrap();
    let found = env.store.find_task("ship").await.unwrap().unwrap();
    assert_eq!(found.blocked.as_deref(), Some(why));
    let clear = |t: Task| Ok(Task { blocked: None, ..t });
    env.store.update_task("ship", clear).await.unwrap();
    let cleared = env.store.find_task("ship").await.unwrap().unwrap();
    assert_eq!(cleared.blocked, None);
}
