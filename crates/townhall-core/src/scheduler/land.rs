use crate::repos::pr_checks::{CheckState, PrFacts};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Verdict {
    Wait,
    Merge,
    Relaunch,
}

pub(super) fn all_green(states: &[CheckState]) -> bool {
    !states.is_empty() && !states.iter().any(|c| c.pending() || c.failed())
}

pub(super) fn decide(states: &[CheckState], facts: Option<&PrFacts>) -> Verdict {
    if states.iter().any(CheckState::pending) {
        return Verdict::Wait;
    }
    if states.is_empty() || states.iter().any(CheckState::failed) {
        return Verdict::Relaunch;
    }
    match facts {
        None => Verdict::Wait,
        Some(f) if f.conflicting || f.behind > 0 => Verdict::Relaunch,
        Some(f) if f.unknown => Verdict::Wait,
        Some(_) => Verdict::Merge,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(name: &str, bucket: &str) -> CheckState {
        CheckState {
            name: name.into(),
            state: bucket.to_uppercase(),
            bucket: bucket.into(),
            link: String::new(),
        }
    }

    fn facts(conflicting: bool, unknown: bool, behind: u64) -> PrFacts {
        PrFacts {
            head: "abc123".into(),
            conflicting,
            unknown,
            behind,
            state: "OPEN".into(),
            merge_state: String::new(),
            merge_commit: None,
        }
    }

    #[test]
    fn green_checks_on_a_clean_pr_merge() {
        let states = [check("build", "pass"), check("lint", "skipping")];
        assert_eq!(
            decide(&states, Some(&facts(false, false, 0))),
            Verdict::Merge
        );
    }

    #[test]
    fn a_failed_or_cancelled_check_relaunches() {
        for bucket in ["fail", "cancel"] {
            let states = [check("build", bucket), check("lint", "pass")];
            assert_eq!(decide(&states, None), Verdict::Relaunch, "{bucket}");
        }
    }

    #[test]
    fn a_pending_check_waits_even_when_another_failed() {
        let states = [check("build", "pending"), check("lint", "fail")];
        assert_eq!(decide(&states, None), Verdict::Wait);
        let states = [check("build", "pending"), check("lint", "pass")];
        assert_eq!(
            decide(&states, Some(&facts(false, false, 0))),
            Verdict::Wait
        );
    }

    #[test]
    fn a_pr_behind_its_base_or_in_conflict_relaunches() {
        let states = [check("build", "pass")];
        assert_eq!(
            decide(&states, Some(&facts(false, false, 3))),
            Verdict::Relaunch
        );
        assert_eq!(
            decide(&states, Some(&facts(true, false, 0))),
            Verdict::Relaunch
        );
    }

    #[test]
    fn an_unknown_mergeability_waits() {
        let states = [check("build", "pass")];
        assert_eq!(decide(&states, Some(&facts(false, true, 0))), Verdict::Wait);
    }

    #[test]
    fn no_checks_never_merge() {
        assert_eq!(
            decide(&[], Some(&facts(false, false, 0))),
            Verdict::Relaunch
        );
    }

    #[test]
    fn green_checks_without_facts_do_not_merge() {
        let states = [check("build", "pass")];
        assert_eq!(decide(&states, None), Verdict::Wait);
    }
}
