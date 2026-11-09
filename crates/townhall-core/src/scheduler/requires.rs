//! An agent type's `requires`: which tasks of the required types sit in a task's `after`
//! chain. Pure, so `create_task` and the scheduler's gate read the chain the same way.

use crate::model::Task;

/// Every task `after` names, and every task those wait for, and so on, once each.
pub fn chain<'a>(tasks: &'a [Task], after: &[String]) -> Vec<&'a Task> {
    let mut seen: Vec<&Task> = vec![];
    let mut pending: Vec<&str> = after.iter().map(String::as_str).collect();
    while let Some(name) = pending.pop() {
        let Some(found) = tasks.iter().find(|t| t.name.eq_ignore_ascii_case(name)) else {
            continue;
        };
        if seen.iter().any(|s| std::ptr::eq(*s, found)) {
            continue;
        }
        seen.push(found);
        pending.extend(found.after.iter().map(String::as_str));
    }
    seen.sort_by_key(|t| tasks.iter().position(|o| std::ptr::eq(o, *t)));
    seen
}

/// The required types no task of `chain` runs as, in the order of `requires`.
pub fn missing<'a>(requires: &'a [String], chain: &[&Task]) -> Vec<&'a str> {
    requires
        .iter()
        .filter(|kind| !chain.iter().any(|t| t.agent.eq_ignore_ascii_case(kind)))
        .map(String::as_str)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::TaskState;

    fn task(name: &str, agent: &str, after: &[&str]) -> Task {
        Task {
            name: name.into(),
            title: name.into(),
            prompt: "p".into(),
            repo: "shop".into(),
            base_branch: "main".into(),
            after: after.iter().map(|a| a.to_string()).collect(),
            state: TaskState::Queued,
            conversation_id: "c".into(),
            attachments: vec![],
            created_at: "2026-11-04T10:42:05.000Z".into(),
            pr: None,
            agent: agent.into(),
            blocked: None,
            pr_base: None,
            done: Default::default(),
        }
    }

    #[test]
    fn the_chain_follows_after_through_every_level_once() {
        let tasks = [
            task("a", "tester", &[]),
            task("b", "reviewer", &["A"]),
            task("c", "builder", &["b", "a"]),
            task("d", "builder", &[]),
        ];
        let names = |after: &[&str]| -> Vec<String> {
            let after: Vec<String> = after.iter().map(|a| a.to_string()).collect();
            chain(&tasks, &after)
                .iter()
                .map(|t| t.name.clone())
                .collect()
        };
        assert_eq!(names(&["c"]), ["a", "b", "c"]);
        assert_eq!(names(&["d", "ghost"]), ["d"]);
        assert!(names(&[]).is_empty());
    }

    #[test]
    fn a_cycle_does_not_loop() {
        let tasks = [task("a", "x", &["b"]), task("b", "x", &["a"])];
        assert_eq!(chain(&tasks, &["a".to_string()]).len(), 2);
    }

    #[test]
    fn missing_names_the_types_no_chain_task_runs_as() {
        let tasks = [task("a", "Tester", &[]), task("b", "builder", &[])];
        let all: Vec<&Task> = tasks.iter().collect();
        let requires = ["tester".to_string(), "reviewer".to_string()];
        assert_eq!(missing(&requires, &all), ["reviewer"]);
        assert_eq!(missing(&requires, &[]), ["tester", "reviewer"]);
        assert!(missing(&[], &all).is_empty());
    }
}
