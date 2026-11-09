use townhall_core::agent_usage::{aggregate, UsageRange, UsageRun};
use townhall_core::clock::from_unix_millis;

mod common;
use common::{new_task, store_env};

const HOUR: i64 = 3_600_000;
const DAY: i64 = 24 * HOUR;
const NOW: i64 = 1_793_836_800_000;

fn run(name: &str, repo: &str, conversation: &str, ago: i64, tokens: u64) -> UsageRun {
    UsageRun {
        name: name.into(),
        repo: repo.into(),
        conversation_id: conversation.into(),
        at_ms: NOW - ago,
        tokens,
    }
}

#[test]
fn ranges_parse_from_their_labels_and_refuse_anything_else() {
    assert_eq!(UsageRange::parse("24h"), Some(UsageRange::Day));
    assert_eq!(UsageRange::parse("7d"), Some(UsageRange::Week));
    assert_eq!(UsageRange::parse("30d"), Some(UsageRange::Month));
    assert_eq!(UsageRange::parse("1y"), None);
    assert_eq!(UsageRange::parse(""), None);
    assert_eq!(UsageRange::Week.label(), "7d");
}

#[test]
fn buckets_cover_the_range_oldest_first_and_end_now() {
    for (range, count, width) in [
        (UsageRange::Day, 24, HOUR),
        (UsageRange::Week, 28, 6 * HOUR),
        (UsageRange::Month, 30, DAY),
    ] {
        let series = aggregate("builder", range, NOW, &[]);
        assert_eq!(series.buckets.len(), count, "{range:?}");
        assert_eq!(series.bucket_ms, width);
        assert_eq!(series.until, from_unix_millis(NOW));
        assert_eq!(
            series.buckets[0].start,
            from_unix_millis(NOW - count as i64 * width)
        );
        assert_eq!(
            series.buckets[count - 1].start,
            from_unix_millis(NOW - width)
        );
        assert!(series.buckets.iter().all(|b| b.runs == 0 && b.tokens == 0));
    }
}

#[test]
fn runs_and_tokens_land_in_the_bucket_of_their_time() {
    let runs = [
        run("a", "shop", "c1", 30 * 60_000, 100),
        run("b", "shop", "c1", 40 * 60_000, 50),
        run("c", "shop", "c2", 5 * HOUR + 1, 7),
    ];
    let series = aggregate("builder", UsageRange::Day, NOW, &runs);
    assert_eq!(
        (series.buckets[23].runs, series.buckets[23].tokens),
        (2, 150)
    );
    assert_eq!((series.buckets[18].runs, series.buckets[18].tokens), (1, 7));
    assert_eq!((series.runs, series.tokens), (3, 157));
}

#[test]
fn runs_outside_the_range_are_left_out() {
    let runs = [
        run("old", "shop", "c1", DAY + 1, 999),
        run("edge", "shop", "c1", DAY, 5),
        run("new", "shop", "c1", 0, 1),
    ];
    let series = aggregate("builder", UsageRange::Day, NOW, &runs);
    assert_eq!((series.runs, series.tokens), (2, 6));
    assert_eq!(series.buckets[0].tokens, 5);
    assert_eq!(series.buckets[23].tokens, 1);
}

#[test]
fn a_run_after_now_counts_in_the_last_bucket() {
    let runs = [run("future", "shop", "c1", -60_000, 3)];
    let series = aggregate("builder", UsageRange::Day, NOW, &runs);
    assert_eq!(series.buckets[23].tokens, 3);
}

#[test]
fn breakdowns_rank_by_tokens_then_runs_then_name_and_keep_the_top_five() {
    let mut runs = vec![
        run("big", "shop", "c1", HOUR, 900),
        run("small", "docs", "c2", HOUR, 10),
        run("tie-b", "docs", "c2", HOUR, 10),
        run("tie-a", "api", "c3", HOUR, 10),
    ];
    runs.extend((0..4).map(|i| run(&format!("x{i}"), "shop", "c1", HOUR, 1)));
    let series = aggregate("builder", UsageRange::Day, NOW, &runs);
    let tasks: Vec<_> = series.by_task.iter().map(|s| s.key.as_str()).collect();
    assert_eq!(tasks, ["big", "small", "tie-a", "tie-b", "x0"]);
    let repos: Vec<_> = series
        .by_repo
        .iter()
        .map(|s| (s.key.as_str(), s.runs, s.tokens))
        .collect();
    assert_eq!(repos, [("shop", 5, 904), ("docs", 2, 20), ("api", 1, 10)]);
    let conversations: Vec<_> = series
        .by_conversation
        .iter()
        .map(|s| s.key.as_str())
        .collect();
    assert_eq!(conversations, ["c1", "c2", "c3"]);
}

#[tokio::test]
async fn the_store_reads_one_agent_types_runs_by_start_time() {
    let env = store_env();
    let c = env.store.create_conversation().await.unwrap();
    let coder = |name: &str| townhall_core::model::NewTask {
        agent: "coder".into(),
        ..new_task(name)
    };
    for task in [new_task("a"), new_task("b"), coder("c")] {
        env.store.create_task(&c.id, task).await.unwrap();
    }
    env.store.set_task_tokens("a", 100).await.unwrap();
    env.store.set_task_tokens("b", 40).await.unwrap();
    env.store.set_task_tokens("c", 9).await.unwrap();
    let (a_at, b_at) = (
        from_unix_millis(NOW - HOUR / 2),
        from_unix_millis(NOW - 3 * DAY),
    );
    env.store
        .call(move |conn| {
            conn.execute("update tasks set created_at = ?1 where name = 'a'", [a_at])?;
            conn.execute("update tasks set created_at = ?1 where name = 'b'", [b_at])?;
            Ok(())
        })
        .await
        .unwrap();

    let day = env
        .store
        .agent_usage_series("BUILDER", UsageRange::Day, NOW)
        .await
        .unwrap();
    assert_eq!(day.agent, "builder");
    assert_eq!((day.runs, day.tokens), (1, 100));
    assert_eq!(day.by_task[0].key, "a");
    assert_eq!(day.by_repo[0].key, "shop");
    assert_eq!(day.by_conversation[0].key, c.id);

    let week = env
        .store
        .agent_usage_series("builder", UsageRange::Week, NOW)
        .await
        .unwrap();
    assert_eq!((week.runs, week.tokens), (2, 140));

    let none = env
        .store
        .agent_usage_series("ghost", UsageRange::Month, NOW)
        .await
        .unwrap();
    assert_eq!((none.runs, none.tokens, none.by_task.len()), (0, 0, 0));
}
