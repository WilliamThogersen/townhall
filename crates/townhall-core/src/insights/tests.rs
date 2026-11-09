use super::*;
use crate::logs::parse_rfc3339;
use crate::model::{AgentEvent, EntryBody, TaskState};

const NOW: &str = "2026-11-06T12:00:00.000Z";

fn now() -> i64 {
    parse_rfc3339(NOW).unwrap()
}

fn entry(id: u64, day: u32, body: EntryBody) -> ThreadEntry {
    ThreadEntry {
        id,
        at: format!("2026-11-{day:02}T10:00:00.000Z"),
        body,
    }
}

fn you(id: u64, day: u32, text: &str) -> ThreadEntry {
    entry(
        id,
        day,
        EntryBody::You {
            text: text.into(),
            attachments: vec![],
            reply_to: None,
        },
    )
}

fn agent(id: u64, day: u32, name: &str, event: AgentEvent, text: &str) -> ThreadEntry {
    entry(
        id,
        day,
        EntryBody::Agent {
            agent: name.into(),
            event,
            text: text.into(),
        },
    )
}

fn convo(id: &str, entries: Vec<ThreadEntry>) -> ConversationData {
    ConversationData {
        id: id.into(),
        entries,
    }
}

fn task(name: &str, agent: &str, state: TaskState, after: &[&str], conv: &str) -> TaskData {
    TaskData {
        task: Task {
            name: name.into(),
            title: name.into(),
            prompt: "p".into(),
            repo: "shop".into(),
            base_branch: "main".into(),
            after: after.iter().map(|s| s.to_string()).collect(),
            state,
            conversation_id: conv.into(),
            attachments: vec![],
            created_at: "2026-11-03T09:00:00.000Z".into(),
            pr: None,
            agent: agent.into(),
            blocked: None,
            pr_base: None,
            done: Default::default(),
        },
        handoffs: vec![],
        ended_at: None,
    }
}

fn handoff(task: &str, from: &str, to: &str, reason: &str, day: u32) -> Handoff {
    Handoff {
        task: task.into(),
        from: from.into(),
        to: to.into(),
        reason: reason.into(),
        summary: "s".into(),
        at: format!("2026-11-{day:02}T11:00:00.000Z"),
    }
}

fn run(data: &InsightData) -> Insights {
    analyze(data, now(), &InsightsArgs::default()).unwrap()
}

fn find<'a>(out: &'a Insights, kind: &str) -> Vec<&'a Pattern> {
    out.patterns.iter().filter(|p| p.kind == kind).collect()
}

#[test]
fn normalizing_lowercases_and_strips_numbers_ids_and_paths() {
    assert_eq!(
        normalize_phrase("Fix the  login bug in src/auth/login.ts, issue 1234!"),
        "fix the login bug in issue"
    );
    assert_eq!(
        normalize_phrase("rerun task fix-2 at 3f2a9c10-1111-2222-3333-444455556666"),
        "rerun task at"
    );
    assert_eq!(normalize_phrase("  /tmp/x 42  "), "");
}

#[test]
fn normalizing_masks_secrets_before_anything_else() {
    let n = normalize_phrase("use token=hunter2hunter2 to push");
    assert!(!n.contains("hunter"), "{n}");
    assert!(n.contains("push"));
}

#[test]
fn repeated_user_requests_group_across_conversations() {
    let data = InsightData {
        conversations: vec![
            convo("c1", vec![you(1, 2, "Please run the tests for PR 12")]),
            convo(
                "c2",
                vec![
                    you(1, 4, "please run the tests for pr 99"),
                    you(2, 5, "something said once"),
                ],
            ),
        ],
        tasks: vec![],
    };
    let out = run(&data);
    let found = find(&out, "user_request");
    assert_eq!(found.len(), 1, "{out:?}");
    let p = found[0];
    assert_eq!(p.pattern, "please run the tests for pr");
    assert_eq!(p.count, 2);
    assert_eq!(p.first_seen, "2026-11-02T10:00:00.000Z");
    assert_eq!(p.last_seen, "2026-11-04T10:00:00.000Z");
    assert_eq!(p.conversations, ["c1", "c2"]);
    assert_eq!(p.samples.len(), 2);
    assert_eq!(p.fingerprint.len(), 16);
    assert!(!p.suggested_automation.is_empty());
    assert_eq!(out.counts.user_messages, 3);
    assert_eq!(out.counts.conversations, 2);
}

#[test]
fn answers_to_questions_are_not_requests() {
    let reply = |id, day| {
        entry(
            id,
            day,
            EntryBody::You {
                text: "yes".into(),
                attachments: vec![],
                reply_to: Some(1),
            },
        )
    };
    let data = InsightData {
        conversations: vec![convo("c1", vec![reply(2, 2), reply(3, 3)])],
        tasks: vec![],
    };
    assert!(run(&data).patterns.is_empty());
}

#[test]
fn needs_you_failed_sent_and_errors_are_counted_per_kind() {
    let needs = |id, day, name: &str| {
        entry(
            id,
            day,
            EntryBody::NeedsYou {
                agent: name.into(),
                question: format!("Run the migrations for {name}?"),
                choices: vec![],
            },
        )
    };
    let err = |id, day, n: u32| {
        entry(
            id,
            day,
            EntryBody::Error {
                message: format!("claude exited with {n}"),
            },
        )
    };
    let data = InsightData {
        conversations: vec![convo(
            "c1",
            vec![
                needs(1, 2, "fix-a"),
                needs(2, 3, "fix-b"),
                agent(
                    3,
                    2,
                    "fix-a",
                    AgentEvent::Failed,
                    "cargo test failed (code 101)",
                ),
                agent(
                    4,
                    3,
                    "fix-b",
                    AgentEvent::Failed,
                    "cargo test failed (code 2)",
                ),
                agent(5, 2, "fix-a", AgentEvent::Sent, "run cargo fmt"),
                agent(6, 3, "fix-b", AgentEvent::Sent, "Run cargo fmt"),
                agent(7, 3, "fix-b", AgentEvent::Passed, "ok"),
                err(8, 2, 1),
                err(9, 4, 137),
            ],
        )],
        tasks: vec![],
    };
    let out = run(&data);
    assert_eq!(find(&out, "needs_you")[0].count, 2);
    assert_eq!(find(&out, "needs_you")[0].tasks, ["fix-a", "fix-b"]);
    assert_eq!(
        find(&out, "agent_failed")[0].pattern,
        "cargo test failed code"
    );
    assert_eq!(find(&out, "manual_step")[0].pattern, "run cargo fmt");
    assert_eq!(
        find(&out, "townhall_error")[0].pattern,
        "claude exited with"
    );
    assert_eq!(out.counts.needs_you, 2);
    assert_eq!(out.counts.agent_failed, 2);
    assert_eq!(out.counts.agent_sent, 2);
    assert_eq!(out.counts.errors, 2);
}

#[test]
fn handoffs_and_task_chains_are_counted_by_agent_type() {
    let mut t1 = task("a-1", "builder", TaskState::Merged, &[], "c1");
    t1.handoffs = vec![
        handoff("a-1", "builder", "tester", "", 2),
        handoff("a-1", "tester", "builder", "Tests fail in 3 files", 3),
    ];
    let mut t2 = task("a-2", "builder", TaskState::Ended, &[], "c1");
    t2.handoffs = vec![
        handoff("a-2", "builder", "tester", "", 4),
        handoff("a-2", "tester", "builder", "tests fail in 7 files", 5),
    ];
    let p = task("a-3", "planner", TaskState::Ended, &[], "c1");
    let b = task("a-4", "builder", TaskState::Error, &["a-3"], "c1");
    let p2 = task("b-3", "planner", TaskState::Ended, &[], "c2");
    let b2 = task("b-4", "builder", TaskState::Ended, &["b-3"], "c2");
    let data = InsightData {
        conversations: vec![],
        tasks: vec![t1, t2, p, b, p2, b2],
    };
    let out = run(&data);

    let handoffs = find(&out, "handoff");
    let plain = handoffs
        .iter()
        .find(|h| h.pattern == "builder -> tester")
        .unwrap_or_else(|| panic!("{out:?}"));
    assert_eq!(plain.count, 2);
    assert_eq!(plain.tasks, ["a-1", "a-2"]);
    let reasoned = handoffs
        .iter()
        .find(|h| h.pattern == "tester -> builder: tests fail in files")
        .unwrap_or_else(|| panic!("{out:?}"));
    assert_eq!(reasoned.count, 2);

    let chains = find(&out, "task_chain");
    assert!(
        chains
            .iter()
            .any(|c| c.pattern == "planner -> builder" && c.count == 2),
        "{chains:?}"
    );
    assert_eq!(out.counts.handoffs, 4);
    assert_eq!(out.counts.tasks, 6);
    assert_eq!(out.counts.tasks_by_agent["builder"]["merged"], 1);
    assert_eq!(out.counts.tasks_by_agent["builder"]["ended"], 2);
    assert_eq!(out.counts.tasks_by_agent["builder"]["error"], 1);
    assert_eq!(out.counts.tasks_by_agent["planner"]["ended"], 2);
}

#[test]
fn old_entries_fall_outside_the_window() {
    let data = InsightData {
        conversations: vec![convo(
            "c1",
            vec![you(1, 1, "same thing"), you(2, 5, "same thing")],
        )],
        tasks: vec![],
    };
    let args = InsightsArgs {
        since: Some("3d".into()),
        top: None,
    };
    assert!(analyze(&data, now(), &args).unwrap().patterns.is_empty());
    let args = InsightsArgs {
        since: Some("2026-11-01T00:00:00Z".into()),
        top: None,
    };
    assert_eq!(analyze(&data, now(), &args).unwrap().patterns.len(), 1);
    let bad = InsightsArgs {
        since: Some("yesterday".into()),
        top: None,
    };
    assert!(analyze(&data, now(), &bad).unwrap_err().contains("since"));
}

#[test]
fn a_handed_back_task_chain_starts_at_the_type_it_began_as() {
    // The store sets `task.agent` to the type handed to, so it is the last stage.
    let handed = |name: &str| {
        let mut t = task(name, "tester", TaskState::Queued, &[], "c1");
        t.handoffs = vec![handoff(name, "coder", "tester", "", 2)];
        t
    };
    let data = InsightData {
        conversations: vec![],
        tasks: vec![handed("a-1"), handed("a-2")],
    };
    let out = run(&data);
    let chains = find(&out, "task_chain");
    assert_eq!(chains.len(), 1, "{out:?}");
    assert_eq!(chains[0].pattern, "coder -> tester");
    assert_eq!(chains[0].count, 2);
}

/// Two copies of every conversation (ids `c<i>` and `c<i>-b`), each with `times` copies of a
/// distinct long request, so every pattern recurs.
fn many_requests(n: usize, times: u32) -> InsightData {
    let word = |mut i: usize| {
        let mut w = String::new();
        loop {
            w.push((b'a' + (i % 26) as u8) as char);
            i /= 26;
            if i == 0 {
                return w;
            }
        }
    };
    let conversations: Vec<ConversationData> = (0..n)
        .map(|i| {
            let entries = (0..times)
                .map(|t| {
                    // the digit differs the raw texts at their start; normalizing drops it
                    let text = format!("{} {} {}", t + 1, word(i), "filler ".repeat(40));
                    you(u64::from(t) + 1, 2 + t, &text)
                })
                .collect();
            convo(&format!("c{i}"), entries)
        })
        .collect();
    let copies = conversations.iter().map(|c| ConversationData {
        id: format!("{}-b", c.id),
        entries: c.entries.clone(),
    });
    InsightData {
        conversations: conversations.iter().cloned().chain(copies).collect(),
        tasks: vec![],
    }
}

#[test]
fn ranking_is_by_count_then_recency_and_ignores_single_occurrences() {
    let data = InsightData {
        conversations: vec![convo(
            "c1",
            vec![
                you(1, 1, "alpha"),
                you(2, 2, "alpha"),
                you(3, 3, "beta"),
                you(4, 4, "beta"),
                you(5, 5, "gamma"),
                you(6, 5, "gamma"),
                you(7, 5, "gamma"),
                you(8, 5, "single"),
            ],
        )],
        tasks: vec![],
    };
    let out = run(&data);
    let names: Vec<(&str, usize)> = out
        .patterns
        .iter()
        .map(|p| (p.pattern.as_str(), p.count))
        .collect();
    assert_eq!(names, [("gamma", 3), ("beta", 2), ("alpha", 2)]);
    assert!(!out.truncated);
}

#[test]
fn top_is_clamped_and_marks_the_output_truncated() {
    let data = many_requests(12, 1);
    let default = run(&data);
    assert_eq!(default.patterns.len(), DEFAULT_TOP);
    assert!(default.truncated);

    let args = |top| InsightsArgs {
        since: None,
        top: Some(top),
    };
    assert_eq!(analyze(&data, now(), &args(3)).unwrap().patterns.len(), 3);
    assert_eq!(analyze(&data, now(), &args(0)).unwrap().patterns.len(), 1);
    let big = many_requests(40, 1);
    assert_eq!(
        analyze(&big, now(), &args(500)).unwrap().patterns.len(),
        MAX_TOP
    );
}

#[test]
fn snippets_ids_and_total_size_are_capped() {
    let args = InsightsArgs {
        since: None,
        top: Some(MAX_TOP),
    };
    let out = analyze(&many_requests(40, 3), now(), &args).unwrap();
    assert!(out.truncated);
    assert!(out.patterns.len() < MAX_TOP, "{}", out.patterns.len());
    assert!(serde_json::to_string(&out).unwrap().len() <= MAX_OUTPUT_BYTES);
    for p in &out.patterns {
        assert_eq!(p.samples.len(), MAX_SAMPLES);
        assert!(p
            .samples
            .iter()
            .all(|s| s.chars().count() <= MAX_SNIPPET_CHARS));
        assert!(p.pattern.chars().count() <= MAX_SNIPPET_CHARS);
        assert!(p.conversations.len() <= MAX_IDS && p.tasks.len() <= MAX_IDS);
    }
}

#[test]
fn ids_are_capped_per_pattern() {
    let conversations = (0..8)
        .map(|i| convo(&format!("c{i}"), vec![you(1, 2, "same thing")]))
        .collect();
    let out = run(&InsightData {
        conversations,
        tasks: vec![],
    });
    assert_eq!(out.patterns[0].count, 8);
    assert_eq!(out.patterns[0].conversations.len(), MAX_IDS);
}

#[test]
fn secrets_never_reach_the_output() {
    let leak = |id, day| {
        you(
            id,
            day,
            "deploy with token=hunter2hunter2 and key sk-ant-api03-abcdefghijklmnopqrstuvwxyz0123456789",
        )
    };
    let failed = |id, day| {
        agent(
            id,
            day,
            "t-1",
            AgentEvent::Failed,
            "auth failed token=hunter2hunter2",
        )
    };
    let mut failing = task("t-1", "builder", TaskState::Error, &[], "c1");
    failing.handoffs = vec![
        handoff(
            "t-1",
            "builder",
            "tester",
            "password=correcthorsebattery",
            2,
        ),
        handoff(
            "t-1",
            "builder",
            "tester",
            "password=correcthorsebattery",
            3,
        ),
    ];
    let data = InsightData {
        conversations: vec![convo(
            "c1",
            vec![leak(1, 2), leak(2, 3), failed(3, 2), failed(4, 3)],
        )],
        tasks: vec![failing],
    };
    let out = run(&data);
    assert_eq!(out.patterns.len(), 3, "{out:?}");
    let json = serde_json::to_string(&out).unwrap();
    for secret in ["hunter2", "sk-ant-api03-abc", "correcthorse"] {
        assert!(!json.contains(secret), "{secret} leaked: {json}");
    }
    assert!(json.contains("REDACTED"), "{json}");
}

#[test]
fn fingerprints_ignore_numbers_case_and_conversation() {
    let at = |conv: &str, texts: [&str; 2]| {
        run(&InsightData {
            conversations: vec![convo(conv, vec![you(1, 2, texts[0]), you(2, 3, texts[1])])],
            tasks: vec![],
        })
    };
    let a = at("c1", ["ship it 1", "ship it 2"]);
    let b = at("zz", ["Ship it 77", "ship it"]);
    assert_eq!(a.patterns[0].fingerprint, b.patterns[0].fingerprint);
    let other = at("c1", ["ship them", "ship them"]);
    assert_ne!(a.patterns[0].fingerprint, other.patterns[0].fingerprint);
    assert_eq!(a.patterns[0].fingerprint.len(), 16);
}
