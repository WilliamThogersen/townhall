//! The scheduler's decisions that need no git, tmux or store: which queued tasks may start,
//! how many slots are free, a task's brief and how long a failure text may be.

use std::path::PathBuf;

use crate::model::{Done, Handoff, Task, TaskState};

/// `ended` or `merged`: a task an `after` list waits for is done. `error` is not: a task
/// waiting for one stays queued.
pub fn finished(state: TaskState) -> bool {
    matches!(state, TaskState::Ended | TaskState::Merged)
}

fn done(tasks: &[Task], name: &str) -> bool {
    tasks
        .iter()
        .any(|t| t.name.eq_ignore_ascii_case(name) && finished(t.state))
}

/// Queued tasks whose every `after` task finished, in the order of `tasks` (oldest first).
pub fn ready(tasks: &[Task]) -> Vec<&Task> {
    tasks
        .iter()
        .filter(|t| t.state == TaskState::Queued)
        .filter(|t| t.after.iter().all(|a| done(tasks, a)))
        .collect()
}

/// How many more task agents may start next to `active` under `limit`.
pub fn free_slots(limit: u32, active: usize) -> usize {
    usize::try_from(limit)
        .unwrap_or(usize::MAX)
        .saturating_sub(active)
}

/// What each hand back told the next agent type, oldest first.
fn handed_back(handoffs: &[Handoff]) -> String {
    let each: Vec<String> = handoffs
        .iter()
        .enumerate()
        .map(|(at, h)| {
            let done = match h.summary.trim().is_empty() {
                true => String::new(),
                false => format!("\n\nWhat was done so far:\n{}", h.summary.trim_end()),
            };
            format!(
                "### {}. handed back by {} to {}\n\nWhy: {}{done}",
                at + 1,
                h.from,
                h.to,
                h.reason
            )
        })
        .collect();
    match each.is_empty() {
        true => String::new(),
        false => format!(
            "\n\n## This task was handed back\n\nThe work is in your worktree and branch already. Fix what the latest hand back says, then finish the task.\n\n{}",
            each.join("\n\n")
        ),
    }
}

/// What finished means and the check that proves it: nothing when the task has neither.
fn done_when(done: &Done) -> String {
    let when = done
        .done_when
        .as_deref()
        .map(str::trim)
        .filter(|w| !w.is_empty());
    let when = when.map(|w| format!("\n\n## Done when\n\n{w}"));
    let check = done.check.as_deref().map(|c| {
        format!(
            "\n\nWhen you finish, townhall runs `{c}` in your worktree. If it fails you are started again with its output."
        )
    });
    [when, check].into_iter().flatten().collect()
}

/// A task's brief: its title as a heading, its prompt, when it is done, what its hand backs
/// said, then its attachments' paths.
pub fn brief_text(
    title: &str,
    prompt: &str,
    done: &Done,
    files: &[PathBuf],
    handoffs: &[Handoff],
) -> String {
    let listed: Vec<String> = files.iter().map(|f| format!("- {}", f.display())).collect();
    let attached = match listed.is_empty() {
        true => String::new(),
        false => format!("\n\nFiles attached to this task:\n{}", listed.join("\n")),
    };
    format!(
        "# {title}\n\n{}{}{}{attached}\n",
        prompt.trim_end(),
        done_when(done),
        handed_back(handoffs)
    )
}

pub fn with_plugin_sections(text: String, sections: &[(String, String)]) -> String {
    if sections.is_empty() {
        return text;
    }
    let mut out = text;
    if !out.ends_with('\n') {
        out.push('\n');
    }
    for (id, body) in sections {
        out.push_str(&format!("\n## From plugin {id}\n\n{}\n", body.trim_end()));
    }
    out
}

/// `text` cut to at most `max` characters, ending in `...` when it was cut.
pub fn capped(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let kept: String = text.chars().take(max.saturating_sub(3)).collect();
    format!("{kept}...")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(name: &str, state: TaskState, after: &[&str]) -> Task {
        Task {
            name: name.into(),
            title: name.into(),
            prompt: "p".into(),
            repo: "shop".into(),
            base_branch: "main".into(),
            after: after.iter().map(|a| a.to_string()).collect(),
            state,
            conversation_id: "c".into(),
            attachments: vec![],
            created_at: "2026-11-04T10:42:05.000Z".into(),
            pr: None,
            agent: "builder".into(),
            blocked: None,
            pr_base: None,
            done: Default::default(),
        }
    }

    #[test]
    fn only_ended_and_merged_count_as_finished() {
        let all = [
            TaskState::Queued,
            TaskState::Working,
            TaskState::NeedsYou,
            TaskState::InCi,
            TaskState::InReview,
            TaskState::Merged,
            TaskState::Ended,
            TaskState::Error,
        ];
        let done: Vec<TaskState> = all.into_iter().filter(|s| finished(*s)).collect();
        assert_eq!(done, [TaskState::Merged, TaskState::Ended]);
    }

    #[test]
    fn ready_keeps_the_order_and_waits_for_every_after_task() {
        let tasks = [
            task("a", TaskState::Ended, &[]),
            task("b", TaskState::Queued, &["A"]),
            task("c", TaskState::Queued, &["a", "d"]),
            task("d", TaskState::Error, &[]),
            task("e", TaskState::Queued, &[]),
            task("f", TaskState::Queued, &["ghost"]),
            task("g", TaskState::Working, &[]),
        ];
        let names: Vec<&str> = ready(&tasks).iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["b", "e"]);
    }

    #[test]
    fn slots_never_go_below_zero() {
        assert_eq!(free_slots(6, 2), 4);
        assert_eq!(free_slots(2, 5), 0);
        assert_eq!(free_slots(1, 0), 1);
    }

    #[test]
    fn the_brief_lists_attachments_after_the_prompt() {
        let none = Done::default();
        assert_eq!(
            brief_text("fix", "do it\n\n", &none, &[], &[]),
            "# fix\n\ndo it\n"
        );
        let files = [PathBuf::from("/h/a.png"), PathBuf::from("/h/b c.txt")];
        assert_eq!(
            brief_text("fix", "do it", &none, &files, &[]),
            "# fix\n\ndo it\n\nFiles attached to this task:\n- /h/a.png\n- /h/b c.txt\n"
        );
    }

    #[test]
    fn the_brief_says_when_the_task_is_done_and_what_checks_it() {
        let done = Done {
            done_when: Some("all tests pass\n".into()),
            check: Some("npm test".into()),
            ..Done::default()
        };
        let files = [PathBuf::from("/a/b.png")];
        assert_eq!(
            brief_text("fix", "do it", &done, &files, &[]),
            "# fix\n\ndo it\n\n## Done when\n\nall tests pass\n\nWhen you finish, townhall runs `npm test` in your worktree. If it fails you are started again with its output.\n\nFiles attached to this task:\n- /a/b.png\n"
        );
        let only_check = Done {
            check: Some("npm test".into()),
            ..Done::default()
        };
        let text = brief_text("fix", "do it", &only_check, &[], &[]);
        assert!(
            !text.contains("Done when") && text.contains("townhall runs `npm test`"),
            "{text}"
        );
        let blank = Done {
            done_when: Some("  ".into()),
            ..Done::default()
        };
        assert_eq!(
            brief_text("fix", "do it", &blank, &[], &[]),
            "# fix\n\ndo it\n"
        );
    }

    #[test]
    fn no_plugin_sections_leave_the_brief_byte_for_byte_alone() {
        let hand = Handoff {
            task: "t".into(),
            from: "a".into(),
            to: "b".into(),
            reason: "why".into(),
            summary: "did".into(),
            at: "2026-11-04T10:42:05.000Z".into(),
        };
        let done = Done {
            done_when: Some("all tests pass".into()),
            check: Some("npm test".into()),
            ..Done::default()
        };
        let files = [PathBuf::from("/a/b.png")];
        for text in [
            brief_text("fix", "do it", &Done::default(), &[], &[]),
            brief_text("fix", "do it\n\n", &done, &files, &[hand.clone()]),
            String::new(),
        ] {
            assert_eq!(with_plugin_sections(text.clone(), &[]), text);
        }
    }

    #[test]
    fn plugin_sections_come_last_each_under_its_own_heading() {
        let files = [PathBuf::from("/a/b.png")];
        let base = brief_text("fix", "do it", &Done::default(), &files, &[]);
        let got = with_plugin_sections(
            base.clone(),
            &[
                ("notes@acme".into(), "write a changelog line".into()),
                ("lint@acme".into(), "run the linter\n\nand fix it".into()),
            ],
        );
        assert_eq!(
            got,
            format!(
                "{base}\n## From plugin notes@acme\n\nwrite a changelog line\n\n## From plugin lint@acme\n\nrun the linter\n\nand fix it\n"
            )
        );
        assert!(got.starts_with(&base));
        assert!(got.find("Files attached").unwrap() < got.find("## From plugin").unwrap());
    }

    #[test]
    fn capped_text_keeps_whole_characters() {
        assert_eq!(capped("short", 300), "short");
        let long = "é".repeat(400);
        let cut = capped(&long, 300);
        assert_eq!(cut.chars().count(), 300);
        assert!(cut.ends_with("..."));
    }
    #[test]
    fn a_brief_lists_each_hand_back_before_the_attachments() {
        let hand = |from: &str, to: &str, why: &str, summary: &str| Handoff {
            task: "t".into(),
            from: from.into(),
            to: to.into(),
            reason: why.into(),
            summary: summary.into(),
            at: "2026-11-04T10:42:05.000Z".into(),
        };
        let handoffs = [
            hand("shipper", "coder", "lint fails", "- feat: add it"),
            hand("coder", "shipper", "fixed", ""),
        ];
        let files = [PathBuf::from("/a/b.png")];
        let text = brief_text("fix", "do it", &Done::default(), &files, &handoffs);
        let at = |needle: &str| {
            text.find(needle)
                .unwrap_or_else(|| panic!("{needle}: {text}"))
        };
        assert!(at("do it") < at("1. handed back by shipper to coder"));
        assert!(at("Why: lint fails") < at("What was done so far:\n- feat: add it"));
        assert!(at("2. handed back by coder to shipper") < at("Files attached"));
        assert!(!text[at("2. handed back")..].contains("What was done"));
    }
}
