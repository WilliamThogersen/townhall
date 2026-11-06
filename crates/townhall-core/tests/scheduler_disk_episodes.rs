use serde_json::json;
use townhall_core::model::{SettingKey, Settings, TaskState};
use townhall_core::scheduler::run_once;

mod scheduler_support;
use scheduler_support::{unique, Sched};

const UNREACHABLE_GB: u32 = 100_000;

async fn set_floor(s: &Sched, gb: u32) {
    let put = s.store().set_setting(SettingKey::MinFreeDiskGb, json!(gb));
    put.await.unwrap();
}

fn told(s: &Sched) -> Vec<String> {
    let waiting = s.ctx().notes().waiting(&s.conversation);
    waiting
        .into_iter()
        .filter(|w| w.starts_with("disk almost full: "))
        .collect()
}

#[tokio::test]
async fn cancelled_while_low_then_recovered_then_a_new_episode_tells_once_more() {
    let s = Sched::new().await;
    set_floor(&s, UNREACHABLE_GB).await;
    let first = unique("ep1");
    s.create(&first, &[]).await;
    run_once(s.ctx()).await.unwrap();
    run_once(s.ctx()).await.unwrap();
    assert_eq!(told(&s).len(), 1);
    assert!(!s.worktree(&first).exists());

    s.set_state(&first, TaskState::Ended).await;
    run_once(s.ctx()).await.unwrap();
    assert_eq!(told(&s).len(), 1);

    set_floor(&s, 0).await;
    run_once(s.ctx()).await.unwrap();

    set_floor(&s, UNREACHABLE_GB).await;
    let (a, b) = (unique("ep2a"), unique("ep2b"));
    s.create(&a, &[]).await;
    s.create(&b, &[]).await;
    for _ in 0..3 {
        run_once(s.ctx()).await.unwrap();
    }
    assert_eq!(told(&s).len(), 2, "one old, exactly one new for two tasks");
    assert_eq!(s.state(&a).await, TaskState::Queued);
    assert_eq!(s.state(&b).await, TaskState::Queued);
    assert!(!s.worktree(&a).exists() && !s.worktree(&b).exists());

    set_floor(&s, 0).await;
    let pass = run_once(s.ctx()).await.unwrap();
    assert_eq!(pass.started.len(), 2, "{pass:?}");
}

#[tokio::test]
async fn a_floor_of_zero_starts_tasks_and_tells_nothing() {
    let s = Sched::new().await;
    set_floor(&s, 0).await;
    let name = unique("off");
    s.create(&name, &[]).await;
    let pass = run_once(s.ctx()).await.unwrap();
    assert_eq!(pass.started, vec![name]);
    assert!(told(&s).is_empty());
}

#[tokio::test]
async fn a_steady_low_disk_warns_with_the_same_text_every_pass() {
    let s = Sched::new().await;
    set_floor(&s, UNREACHABLE_GB).await;
    let name = unique("steady");
    s.create(&name, &[]).await;
    let first = run_once(s.ctx()).await.unwrap().warnings;
    let second = run_once(s.ctx()).await.unwrap().warnings;
    assert_eq!(first.len(), 1, "{first:?}");
    assert_eq!(first, second);
    assert!(!first[0].contains(" free"), "{first:?}");
}

#[tokio::test]
async fn a_retired_worktree_takes_its_own_target_folder_with_it() {
    let s = Sched::new().await;
    set_floor(&s, 0).await;
    let name = unique("target");
    s.create(&name, &[]).await;
    run_once(s.ctx()).await.unwrap();
    let target = s.worktree(&name).join("target/debug");
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(target.join("artifact"), "built").unwrap();
    townhall_core::agent::retire(&s.ws, &name).await.unwrap();
    assert!(!s.worktree(&name).exists());
}

#[tokio::test]
async fn the_harness_floor_is_off_and_the_production_default_is_not() {
    assert_eq!(Settings::default().min_free_disk_gb, 5);
    let s = Sched::new().await;
    let settings = s.ctx().settings().await.unwrap();
    assert_eq!(settings.min_free_disk_gb, 0);
    let name = unique("harness");
    s.create(&name, &[]).await;
    let pass = run_once(s.ctx()).await.unwrap();
    assert_eq!(pass.started, vec![name]);
    assert!(pass.warnings.is_empty(), "{:?}", pass.warnings);
}
