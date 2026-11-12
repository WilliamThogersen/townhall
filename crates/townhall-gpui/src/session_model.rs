use townhall_core::model::{ContextUsage, ConversationDetail, EntryBody, RepoRule};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoLine {
    pub label: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContextView {
    pub percent: u32,
    pub high: bool,
    pub words: String,
}

pub const HIGH_CONTEXT_PERCENT: f64 = 80.0;

fn rule_reason(rule: RepoRule) -> &'static str {
    match rule {
        RepoRule::Named => "you named it",
        RepoRule::Code => "found in its code",
        RepoRule::Only => "the only repo",
        RepoRule::Picked => "you picked it",
        RepoRule::Linked => "set by hand",
    }
}

fn same_repo(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

fn label(repo: &str, base_branch: Option<&str>) -> String {
    match base_branch {
        Some(branch) if !branch.is_empty() => format!("~ {repo} · {branch}"),
        _ => format!("~ {repo}"),
    }
}

pub fn repo_line(detail: &ConversationDetail) -> Option<RepoLine> {
    let newest = detail
        .entries
        .iter()
        .filter_map(|entry| match &entry.body {
            EntryBody::Repo {
                repo,
                base_branch,
                rule,
                reason,
            } => Some((entry.id, repo, base_branch, rule, reason)),
            _ => None,
        })
        .max_by_key(|(id, ..)| *id);
    let linked = detail
        .conversation
        .linked_repo
        .as_deref()
        .filter(|l| !l.is_empty());
    if let Some(linked) = linked {
        if newest.is_none_or(|(_, repo, ..)| !same_repo(repo, linked)) {
            let base = detail
                .tasks
                .iter()
                .find(|t| same_repo(&t.repo, linked))
                .map(|t| t.base_branch.as_str());
            return Some(RepoLine {
                label: label(linked, base),
                reason: rule_reason(RepoRule::Linked).to_string(),
            });
        }
    }
    let (_, repo, base_branch, rule, reason) = newest?;
    let reason = match reason.trim() {
        "" => rule_reason(*rule).to_string(),
        text => text.to_string(),
    };
    Some(RepoLine {
        label: label(repo, Some(base_branch)),
        reason,
    })
}

const THOUSAND: i64 = 1_000;
const MILLION: i64 = 1_000_000;

pub fn format_tokens(n: i64) -> String {
    let thousands = (n as f64 / THOUSAND as f64).round() as i64;
    if n < THOUSAND {
        return n.max(0).to_string();
    }
    if thousands < THOUSAND {
        return format!("{thousands}k");
    }
    let tenths = (n as f64 / (MILLION / 10) as f64).round() / 10.0;
    format!("{tenths}m")
}

pub fn context_view(usage: Option<&ContextUsage>) -> Option<ContextView> {
    let usage = usage?;
    let exact = if usage.limit == 0 {
        0.0
    } else {
        (usage.used as f64 / usage.limit as f64 * 100.0).clamp(0.0, 100.0)
    };
    Some(ContextView {
        percent: exact.floor() as u32,
        high: exact >= HIGH_CONTEXT_PERCENT,
        words: format!(
            "{} of {} tokens",
            format_tokens(usage.used as i64),
            format_tokens(usage.limit as i64)
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::{conversation, detail_of, entry_of, task, you};
    use townhall_core::model::{EntryBody, RepoRule, TaskState, ThreadEntry};

    fn repo_entry(id: u64, repo: &str, rule: RepoRule, reason: &str, base: &str) -> ThreadEntry {
        entry_of(
            id,
            EntryBody::Repo {
                repo: repo.into(),
                base_branch: base.into(),
                rule,
                reason: reason.into(),
            },
        )
    }

    fn conv() -> townhall_core::model::Conversation {
        conversation("c", "t", "2026-11-12T10:00:00.000Z")
    }

    fn line(label: &str, reason: &str) -> Option<RepoLine> {
        Some(RepoLine {
            label: label.into(),
            reason: reason.into(),
        })
    }

    #[test]
    fn uses_the_newest_repo_entry_and_its_reason() {
        let detail = detail_of(
            conv(),
            vec![
                repo_entry(2, "shop", RepoRule::Named, "you named it", "develop"),
                you(3, "now the search"),
                repo_entry(
                    4,
                    "townhall",
                    RepoRule::Code,
                    "it has src/search/query.ts",
                    "main",
                ),
                you(5, "thanks"),
            ],
            vec![],
        );
        assert_eq!(
            repo_line(&detail),
            line("~ townhall · main", "it has src/search/query.ts")
        );
    }

    #[test]
    fn picks_the_highest_id_not_the_last_in_the_list() {
        let detail = detail_of(
            conv(),
            vec![
                repo_entry(9, "townhall", RepoRule::Only, "only repo", "main"),
                repo_entry(2, "shop", RepoRule::Named, "you named it", "main"),
            ],
            vec![],
        );
        assert_eq!(repo_line(&detail).unwrap().label, "~ townhall · main");
    }

    #[test]
    fn says_why_for_an_entry_without_a_reason() {
        let table = [
            (RepoRule::Named, "you named it"),
            (RepoRule::Code, "found in its code"),
            (RepoRule::Only, "the only repo"),
            (RepoRule::Picked, "you picked it"),
            (RepoRule::Linked, "set by hand"),
        ];
        for (rule, reason) in table {
            let detail = detail_of(
                conv(),
                vec![repo_entry(1, "shop", rule, "  ", "main")],
                vec![],
            );
            assert_eq!(repo_line(&detail).unwrap().reason, reason, "{rule:?}");
        }
    }

    #[test]
    fn says_set_by_hand_when_the_linked_repo_differs_from_the_newest_entry() {
        let mut c = conv();
        c.linked_repo = Some("shop".into());
        let mut t = task("t", TaskState::Working);
        t.repo = "Shop".into();
        t.base_branch = "develop".into();
        let detail = detail_of(
            c,
            vec![repo_entry(
                1,
                "townhall",
                RepoRule::Named,
                "you named it",
                "main",
            )],
            vec![t],
        );
        assert_eq!(repo_line(&detail), line("~ shop · develop", "set by hand"));
    }

    #[test]
    fn keeps_the_entry_when_it_names_the_linked_repo() {
        let mut c = conv();
        c.linked_repo = Some("Townhall".into());
        let detail = detail_of(
            c,
            vec![repo_entry(
                1,
                "townhall",
                RepoRule::Linked,
                "you linked it",
                "main",
            )],
            vec![],
        );
        assert_eq!(
            repo_line(&detail),
            line("~ townhall · main", "you linked it")
        );
    }

    #[test]
    fn leaves_the_branch_out_of_a_linked_repo_no_task_has_used() {
        let mut c = conv();
        c.linked_repo = Some("shop".into());
        let detail = detail_of(c, vec![], vec![]);
        assert_eq!(repo_line(&detail), line("~ shop", "set by hand"));
    }

    #[test]
    fn is_none_before_townhall_picked_a_repo() {
        let detail = detail_of(conv(), vec![you(1, "hi")], vec![]);
        assert_eq!(repo_line(&detail), None);
    }

    #[test]
    fn tokens_are_written_short() {
        let table: &[(i64, &str)] = &[
            (0, "0"),
            (999, "999"),
            (1000, "1k"),
            (56_400, "56k"),
            (200_000, "200k"),
            (999_499, "999k"),
            (1_000_000, "1m"),
            (1_250_000, "1.3m"),
        ];
        for (n, text) in table {
            assert_eq!(format_tokens(*n), *text, "{n}");
        }
    }

    fn usage(used: u64, limit: u64) -> Option<ContextUsage> {
        Some(ContextUsage { used, limit })
    }

    #[test]
    fn there_is_no_view_before_the_session_has_context() {
        assert_eq!(context_view(None), None);
    }

    #[test]
    fn the_view_gives_percent_words_and_a_normal_level_under_the_threshold() {
        assert_eq!(
            context_view(usage(56_000, 200_000).as_ref()),
            Some(ContextView {
                percent: 28,
                high: false,
                words: "56k of 200k tokens".into()
            })
        );
    }

    #[test]
    fn the_level_turns_high_at_80_percent() {
        let high = |used| context_view(usage(used, 200_000).as_ref()).unwrap().high;
        assert!(!high(159_000));
        assert!(high(160_000));
    }

    #[test]
    fn the_percent_stays_between_0_and_100() {
        let percent = |used, limit| context_view(usage(used, limit).as_ref()).unwrap().percent;
        assert_eq!(percent(300_000, 200_000), 100);
        assert_eq!(percent(10, 0), 0);
    }
}
