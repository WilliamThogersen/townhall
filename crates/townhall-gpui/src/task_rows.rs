use townhall_core::model::{Checks, Task, TaskState, VerifyPhase};

use crate::glyph::Glyph;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskTone {
    Waiting,
    Error,
    Merged,
    Dead,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskView {
    pub task: Task,
    pub glyph: Glyph,
    pub tone: Option<TaskTone>,
    pub words: String,
    pub blocked: Option<String>,
    pub pr_open: bool,
    pub pr_target: Option<String>,
}

#[derive(Clone, Copy)]
struct Look {
    glyph: Glyph,
    tone: Option<TaskTone>,
    words: &'static str,
    rank: u8,
}

fn look_of_state(state: TaskState) -> Look {
    let look = |glyph, tone, words, rank| Look {
        glyph,
        tone,
        words,
        rank,
    };
    match state {
        TaskState::NeedsYou => look(Glyph::Waiting, Some(TaskTone::Waiting), "needs you", 0),
        TaskState::Error => look(Glyph::Error, Some(TaskTone::Error), "failed", 1),
        TaskState::Working => look(Glyph::Working, None, "working", 2),
        TaskState::Queued => look(Glyph::Queued, None, "queued", 3),
        TaskState::InCi => look(Glyph::Ci, None, "in ci", 4),
        TaskState::InReview => look(Glyph::Ci, None, "in review", 5),
        TaskState::Merged => look(Glyph::Merged, Some(TaskTone::Merged), "merged", 6),
        TaskState::Ended => look(Glyph::Dead, Some(TaskTone::Dead), "ended", 7),
    }
}

const SUMMARY_ORDER: [TaskState; 8] = [
    TaskState::NeedsYou,
    TaskState::Error,
    TaskState::Working,
    TaskState::Queued,
    TaskState::InCi,
    TaskState::InReview,
    TaskState::Merged,
    TaskState::Ended,
];

fn finished(state: TaskState) -> bool {
    matches!(state, TaskState::Ended | TaskState::Merged)
}

fn same_name(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

fn look_of(task: &Task) -> Look {
    let checks = task.pr.as_ref().map(|pr| pr.checks);
    let base = look_of_state(task.state);
    match (checks, task.state) {
        (Some(Checks::Failed), TaskState::InCi | TaskState::Error) => Look {
            glyph: Glyph::Error,
            tone: Some(TaskTone::Error),
            words: "checks failed",
            rank: look_of_state(TaskState::Error).rank,
        },
        (Some(Checks::Passed), TaskState::InCi) => Look {
            words: "checks passed",
            ..base
        },
        _ => base,
    }
}

fn retried(retries: u32) -> String {
    format!(
        "{retries} {}",
        if retries == 1 { "retry" } else { "retries" }
    )
}

pub fn verify_words(task: &Task) -> Option<String> {
    let verify = task.done.verify.as_ref()?;
    task.done.check.as_ref()?;
    let retry = format!("retry {}/{}", verify.retries, task.done.max_retries);
    match verify.phase {
        VerifyPhase::Verifying if verify.retries > 0 => Some(format!("verifying · {retry}")),
        VerifyPhase::Verifying => Some("verifying".to_string()),
        VerifyPhase::Retrying => Some(retry),
        VerifyPhase::Failed if verify.retries > 0 => {
            Some(format!("check failed after {}", retried(verify.retries)))
        }
        VerifyPhase::Failed => Some("check failed".to_string()),
        VerifyPhase::Passed if task.state == TaskState::Ended => Some("check passed".to_string()),
        VerifyPhase::Passed => None,
    }
}

pub fn pending_after(task: &Task, siblings: &[Task]) -> Vec<String> {
    task.after
        .iter()
        .filter(
            |name| match siblings.iter().find(|other| same_name(&other.name, name)) {
                Some(found) => !finished(found.state),
                None => true,
            },
        )
        .cloned()
        .collect()
}

fn blocked_of(task: &Task, siblings: &[Task]) -> Option<String> {
    if task.state != TaskState::Queued || !pending_after(task, siblings).is_empty() {
        return None;
    }
    task.blocked.clone()
}

fn words_of(task: &Task, look: Look, siblings: &[Task], blocked: &Option<String>) -> String {
    if let Some(checked) = verify_words(task) {
        return checked;
    }
    if task.state != TaskState::Queued {
        return look.words.to_string();
    }
    let waiting = pending_after(task, siblings);
    if !waiting.is_empty() {
        return format!("after {}", waiting.join(", "));
    }
    if blocked.is_some() {
        "blocked".to_string()
    } else {
        look.words.to_string()
    }
}

pub fn task_view(task: &Task, siblings: &[Task]) -> TaskView {
    let look = look_of(task);
    let blocked = blocked_of(task, siblings);
    TaskView {
        task: task.clone(),
        glyph: look.glyph,
        tone: look.tone,
        words: words_of(task, look, siblings, &blocked),
        blocked,
        pr_open: task.pr.is_some() && !finished(task.state),
        pr_target: task
            .pr_base
            .as_ref()
            .filter(|base| **base != task.base_branch)
            .cloned(),
    }
}

pub fn sort_tasks(tasks: &[Task]) -> Vec<Task> {
    let mut sorted = tasks.to_vec();
    sorted.sort_by_key(|t| look_of(t).rank);
    sorted
}

pub fn task_views(tasks: &[Task]) -> Vec<TaskView> {
    sort_tasks(tasks)
        .iter()
        .map(|task| task_view(task, tasks))
        .collect()
}

pub fn task_count(n: usize) -> String {
    format!("{n} {}", if n == 1 { "task" } else { "tasks" })
}

pub fn task_summary(tasks: &[Task]) -> String {
    let mut parts = vec![task_count(tasks.len())];
    for state in SUMMARY_ORDER {
        let count = tasks.iter().filter(|t| t.state == state).count();
        if count > 0 {
            parts.push(format!("{count} {}", look_of_state(state).words));
        }
    }
    parts.join(" · ")
}

pub fn same_task(a: Option<&str>, b: &str) -> bool {
    a.is_some_and(|a| same_name(a, b))
}

pub fn task_words(task: &Task, tasks: &[Task]) -> String {
    task_view(task, tasks).words
}

pub fn unfinished_count(tasks: &[Task]) -> usize {
    tasks
        .iter()
        .filter(|t| {
            !matches!(
                t.state,
                TaskState::Merged | TaskState::Ended | TaskState::Error
            )
        })
        .count()
}

pub fn needs_you_count(tasks: &[Task]) -> usize {
    tasks
        .iter()
        .filter(|t| t.state == TaskState::NeedsYou)
        .count()
}

pub fn task_block_head(tasks: &[Task], count: usize) -> String {
    let mut repos: Vec<&str> = Vec::new();
    for task in tasks {
        if !repos.contains(&task.repo.as_str()) {
            repos.push(&task.repo);
        }
    }
    let head = task_count(count);
    match repos.as_slice() {
        [only] => match tasks.first().map(|t| t.base_branch.as_str()) {
            Some(branch) if !branch.is_empty() => format!("{head} for {only} · {branch}"),
            _ => format!("{head} for {only}"),
        },
        [] => head,
        many => format!("{head} for {}", many.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::task;
    use townhall_core::model::{Checks, Pr, TaskState, Verify, VerifyPhase};

    fn pr(checks: Checks) -> Pr {
        Pr {
            number: 41,
            url: Some("https://git.test/pr/41".into()),
            checks,
        }
    }

    fn named(name: &str, state: TaskState) -> Task {
        task(name, state)
    }

    fn names(tasks: &[Task]) -> Vec<&str> {
        tasks.iter().map(|t| t.name.as_str()).collect()
    }

    fn verify(phase: VerifyPhase, retries: u32) -> Verify {
        Verify {
            phase,
            retries,
            output: None,
        }
    }

    fn checked(state: TaskState, v: Verify) -> Task {
        let mut t = named("t", state);
        t.done.check = Some("npm test".into());
        t.done.verify = Some(v);
        t
    }

    #[test]
    fn a_task_is_worded_by_where_its_check_stands() {
        let words = |state, v| task_view(&checked(state, v), &[]).words;
        assert_eq!(
            words(TaskState::Working, verify(VerifyPhase::Verifying, 0)),
            "verifying"
        );
        assert_eq!(
            words(TaskState::Working, verify(VerifyPhase::Verifying, 1)),
            "verifying · retry 1/2"
        );
        assert_eq!(
            words(TaskState::Working, verify(VerifyPhase::Retrying, 1)),
            "retry 1/2"
        );
        assert_eq!(
            words(TaskState::Queued, verify(VerifyPhase::Retrying, 1)),
            "retry 1/2"
        );
        assert_eq!(
            words(TaskState::NeedsYou, verify(VerifyPhase::Failed, 2)),
            "check failed after 2 retries"
        );
        assert_eq!(
            words(TaskState::NeedsYou, verify(VerifyPhase::Failed, 1)),
            "check failed after 1 retry"
        );
        assert_eq!(
            words(TaskState::NeedsYou, verify(VerifyPhase::Failed, 0)),
            "check failed"
        );
        assert_eq!(
            words(TaskState::Ended, verify(VerifyPhase::Passed, 0)),
            "check passed"
        );
    }

    #[test]
    fn a_task_without_a_check_keeps_the_plain_words() {
        assert_eq!(
            task_view(&named("t", TaskState::Working), &[]).words,
            "working"
        );
        assert_eq!(
            task_view(&named("t", TaskState::NeedsYou), &[]).words,
            "needs you"
        );
        assert_eq!(verify_words(&named("t", TaskState::Ended)), None);
    }

    #[test]
    fn needs_you_first_and_finished_work_last() {
        let tasks = vec![
            named("ended-1", TaskState::Ended),
            named("merged-1", TaskState::Merged),
            named("ci-1", TaskState::InCi),
            named("queued-1", TaskState::Queued),
            named("working-1", TaskState::Working),
            named("error-1", TaskState::Error),
            named("needs-1", TaskState::NeedsYou),
        ];
        assert_eq!(
            names(&sort_tasks(&tasks)),
            [
                "needs-1",
                "error-1",
                "working-1",
                "queued-1",
                "ci-1",
                "merged-1",
                "ended-1"
            ]
        );
    }

    #[test]
    fn sorting_keeps_insertion_order_inside_a_group() {
        let tasks = vec![
            named("w-a", TaskState::Working),
            named("q-a", TaskState::Queued),
            named("w-b", TaskState::Working),
            named("w-c", TaskState::Working),
        ];
        assert_eq!(names(&sort_tasks(&tasks)), ["w-a", "w-b", "w-c", "q-a"]);
    }

    #[test]
    fn in_review_sorts_after_in_ci_and_before_merged() {
        let tasks = vec![
            named("m", TaskState::Merged),
            named("r", TaskState::InReview),
            named("c", TaskState::InCi),
        ];
        assert_eq!(names(&sort_tasks(&tasks)), ["c", "r", "m"]);
    }

    #[test]
    fn failed_checks_rank_with_the_errors() {
        let mut ci = named("ci", TaskState::InCi);
        ci.pr = Some(pr(Checks::Failed));
        let tasks = vec![
            named("w", TaskState::Working),
            ci,
            named("e", TaskState::Error),
        ];
        assert_eq!(names(&sort_tasks(&tasks)), ["ci", "e", "w"]);
    }

    #[test]
    fn every_state_has_words_a_glyph_and_a_tone() {
        let table = [
            (
                TaskState::NeedsYou,
                "needs you",
                Glyph::Waiting,
                Some(TaskTone::Waiting),
            ),
            (TaskState::Working, "working", Glyph::Working, None),
            (TaskState::Queued, "queued", Glyph::Queued, None),
            (TaskState::InCi, "in ci", Glyph::Ci, None),
            (TaskState::InReview, "in review", Glyph::Ci, None),
            (
                TaskState::Merged,
                "merged",
                Glyph::Merged,
                Some(TaskTone::Merged),
            ),
            (TaskState::Ended, "ended", Glyph::Dead, Some(TaskTone::Dead)),
            (
                TaskState::Error,
                "failed",
                Glyph::Error,
                Some(TaskTone::Error),
            ),
        ];
        for (state, words, glyph, tone) in table {
            let t = named("t", state);
            let v = task_view(&t, std::slice::from_ref(&t));
            assert_eq!(
                (v.words.as_str(), v.glyph, v.tone),
                (words, glyph, tone),
                "{state:?}"
            );
        }
    }

    #[test]
    fn a_queued_task_names_the_unfinished_task_it_waits_for() {
        let login = named("fix-login", TaskState::Working);
        let mut cookie = named("logout-cookie", TaskState::Queued);
        cookie.after = vec!["fix-login".into()];
        let all = [login, cookie.clone()];
        assert_eq!(task_view(&cookie, &all).words, "after fix-login");
    }

    #[test]
    fn finished_tasks_are_left_out_of_the_after_words() {
        let done = named("session-ttl", TaskState::Merged);
        let ended = named("auth-tests", TaskState::Ended);
        let mut cookie = named("logout-cookie", TaskState::Queued);
        cookie.after = vec!["session-ttl".into(), "auth-tests".into()];
        let all = [done, ended, cookie.clone()];
        assert_eq!(task_view(&cookie, &all).words, "queued");
    }

    #[test]
    fn an_after_task_it_does_not_know_counts_as_unfinished() {
        let mut cookie = named("logout-cookie", TaskState::Queued);
        cookie.after = vec!["fix-login".into(), "other".into()];
        assert_eq!(
            task_view(&cookie, std::slice::from_ref(&cookie)).words,
            "after fix-login, other"
        );
    }

    #[test]
    fn after_names_match_ignoring_case() {
        let login = named("Fix-Login", TaskState::Ended);
        let mut cookie = named("logout-cookie", TaskState::Queued);
        cookie.after = vec!["fix-login".into()];
        let all = [login, cookie.clone()];
        assert_eq!(task_view(&cookie, &all).words, "queued");
    }

    #[test]
    fn a_queued_task_held_back_by_its_requirements_says_blocked_with_the_reason() {
        let why = "`rev-1` (reviewer) reported REJECT";
        let mut held = named("ship", TaskState::Queued);
        held.blocked = Some(why.into());
        let v = task_view(&held, std::slice::from_ref(&held));
        assert_eq!(v.words, "blocked");
        assert_eq!(v.blocked.as_deref(), Some(why));
        assert_eq!(v.glyph, Glyph::Queued);
        assert_eq!(v.tone, None);
        let free = named("ship", TaskState::Queued);
        assert_eq!(task_view(&free, &[]).blocked, None);
    }

    #[test]
    fn the_unfinished_after_task_is_named_before_a_stale_block_reason() {
        let login = named("fix-login", TaskState::Working);
        let mut held = named("ship", TaskState::Queued);
        held.after = vec!["fix-login".into()];
        held.blocked = Some("old reason".into());
        let v = task_view(&held, &[login, held.clone()]);
        assert_eq!(v.words, "after fix-login");
        assert_eq!(v.blocked, None);
    }

    #[test]
    fn the_pr_target_is_named_only_when_it_is_not_the_base_branch() {
        let mut shipper = named("ship", TaskState::Queued);
        shipper.base_branch = "townhall/build".into();
        shipper.pr_base = Some("main".into());
        assert_eq!(task_view(&shipper, &[]).pr_target.as_deref(), Some("main"));
        let mut same = named("ship", TaskState::Queued);
        same.pr_base = Some("main".into());
        assert_eq!(task_view(&same, &[]).pr_target, None);
        assert_eq!(
            task_view(&named("ship", TaskState::Queued), &[]).pr_target,
            None
        );
    }

    #[test]
    fn no_block_reason_is_shown_once_a_task_is_past_the_queue() {
        let mut t = named("ship", TaskState::Working);
        t.blocked = Some("old reason".into());
        let v = task_view(&t, &[]);
        assert_eq!((v.words.as_str(), v.blocked), ("working", None));
    }

    #[test]
    fn failed_checks_read_checks_failed_in_red() {
        for state in [TaskState::InCi, TaskState::Error] {
            let mut t = named("guard", state);
            t.pr = Some(pr(Checks::Failed));
            let v = task_view(&t, &[]);
            assert_eq!(
                (v.words.as_str(), v.glyph, v.tone),
                ("checks failed", Glyph::Error, Some(TaskTone::Error))
            );
        }
        let mut error = named("guard", TaskState::Error);
        error.pr = Some(pr(Checks::Failed));
        assert!(task_view(&error, &[]).pr_open);
    }

    #[test]
    fn the_failed_checks_words_stay_with_tasks_that_are_still_open() {
        let mut merged = named("guard", TaskState::Merged);
        merged.pr = Some(pr(Checks::Failed));
        assert_eq!(task_view(&merged, &[]).words, "merged");
        let mut working = named("guard", TaskState::Working);
        working.pr = Some(pr(Checks::Failed));
        assert_eq!(task_view(&working, &[]).words, "working");
    }

    #[test]
    fn a_green_pull_request_in_ci_says_checks_passed() {
        let mut t = named("guard", TaskState::InCi);
        t.pr = Some(pr(Checks::Passed));
        assert_eq!(task_view(&t, &[]).words, "checks passed");
    }

    #[test]
    fn an_open_pull_request_is_tagged_and_a_merged_one_is_not() {
        let open = |state| {
            let mut t = named("a", state);
            t.pr = Some(pr(Checks::Pending));
            task_view(&t, &[]).pr_open
        };
        assert!(open(TaskState::InReview));
        assert!(!open(TaskState::Merged));
        assert!(!open(TaskState::Ended));
        assert!(!task_view(&named("d", TaskState::Working), &[]).pr_open);
    }

    #[test]
    fn pending_after_keeps_the_order_the_task_lists_them_in() {
        let mut t = named("x", TaskState::Queued);
        t.after = vec!["b".into(), "a".into()];
        assert_eq!(pending_after(&t, std::slice::from_ref(&t)), ["b", "a"]);
    }

    #[test]
    fn the_summary_leaves_zero_parts_out() {
        let tasks = vec![
            named("a", TaskState::NeedsYou),
            named("b", TaskState::Working),
            named("c", TaskState::Ended),
            named("d", TaskState::Ended),
            named("e", TaskState::Ended),
        ];
        assert_eq!(
            task_summary(&tasks),
            "5 tasks · 1 needs you · 1 working · 3 ended"
        );
    }

    #[test]
    fn the_summary_orders_the_parts_by_attention() {
        let tasks = vec![
            named("a", TaskState::Merged),
            named("b", TaskState::InReview),
            named("c", TaskState::InCi),
            named("d", TaskState::Queued),
            named("e", TaskState::Working),
            named("f", TaskState::Error),
            named("g", TaskState::NeedsYou),
        ];
        assert_eq!(
            task_summary(&tasks),
            "7 tasks · 1 needs you · 1 failed · 1 working · 1 queued · 1 in ci · 1 in review · 1 merged"
        );
    }

    #[test]
    fn the_summary_says_task_for_one_and_tasks_for_none() {
        assert_eq!(
            task_summary(&[named("a", TaskState::Working)]),
            "1 task · 1 working"
        );
        assert_eq!(task_summary(&[]), "0 tasks");
        assert_eq!(task_count(1), "1 task");
    }

    #[test]
    fn same_task_ignores_case_and_needs_a_name() {
        assert!(same_task(Some("Fix"), "fix"));
        assert!(!same_task(None, "fix"));
    }

    #[test]
    fn task_words_follow_the_session_panel_for_every_task() {
        let mut green = named("green", TaskState::InCi);
        green.pr = Some(pr(Checks::Passed));
        let mut red = named("red", TaskState::Error);
        red.pr = Some(pr(Checks::Failed));
        let mut logout = named("logout", TaskState::Queued);
        logout.after = vec!["fix-login".into()];
        let tasks = vec![
            named("Fix-Login", TaskState::Working),
            named("broken", TaskState::Error),
            green,
            red,
            logout,
        ];
        let words: Vec<String> = tasks.iter().map(|t| task_words(t, &tasks)).collect();
        assert_eq!(
            words,
            [
                "working",
                "failed",
                "checks passed",
                "checks failed",
                "after fix-login"
            ]
        );
    }

    #[test]
    fn counts_of_unfinished_and_needs_you_tasks() {
        let tasks = vec![
            named("a", TaskState::NeedsYou),
            named("b", TaskState::Working),
            named("c", TaskState::Merged),
            named("d", TaskState::Error),
            named("e", TaskState::Queued),
        ];
        assert_eq!(unfinished_count(&tasks), 3);
        assert_eq!(needs_you_count(&tasks), 1);
    }

    #[test]
    fn the_task_block_head_names_the_repo_and_branch() {
        let mut a = named("a", TaskState::Queued);
        a.repo = "townhall".into();
        let mut b = named("b", TaskState::Queued);
        b.repo = "townhall".into();
        assert_eq!(task_block_head(&[a, b], 2), "2 tasks for townhall · main");
        let mut s = named("a", TaskState::Queued);
        s.repo = "shop".into();
        let mut t = named("b", TaskState::Queued);
        t.repo = "townhall".into();
        assert_eq!(task_block_head(&[s, t], 3), "3 tasks for shop, townhall");
        assert_eq!(task_block_head(&[], 1), "1 task");
    }
}
