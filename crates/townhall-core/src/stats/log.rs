//! Parsing `git log` output: merge subjects, conventional types and numstat.

/// Starts each record of [`MERGE_FORMAT`] output.
pub(super) const RECORD: char = '\u{1e}';
/// `git log --format` for merges: parents, committer date, subject; numstat lines follow.
pub(super) const MERGE_FORMAT: &str = "%x1e%P%x09%cd%x09%s";
const BRANCH_PREFIX: &str = "townhall/";

/// A `townhall/*` merge on the base branch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Merge {
    pub date: String,
    pub first: String,
    pub second: String,
    pub added: u64,
}

/// The conventional type of a commit subject, as far as stats care.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Fix,
    Feature,
    Other,
}

/// `fix`, `feat` or other, with an optional scope and `!`, in any case.
pub(super) fn kind(subject: &str) -> Kind {
    let Some((head, _)) = subject.split_once(':') else {
        return Kind::Other;
    };
    let head = head.strip_suffix('!').unwrap_or(head);
    let kind = match head.split_once('(') {
        Some((kind, scope)) if scope.ends_with(')') => kind,
        Some(_) => return Kind::Other,
        None => head,
    };
    if kind.eq_ignore_ascii_case("fix") {
        Kind::Fix
    } else if kind.eq_ignore_ascii_case("feat") {
        Kind::Feature
    } else {
        Kind::Other
    }
}

/// The `<name>` of the `townhall/<name>` branch a merge subject names: git's
/// `Merge branch 'townhall/x'` (also remote-tracking) and GitHub's
/// `Merge pull request #n from owner/townhall/x`.
pub(super) fn merged_branch(subject: &str) -> Option<&str> {
    let quoted = subject
        .strip_prefix("Merge branch '")
        .or_else(|| subject.strip_prefix("Merge remote-tracking branch '"));
    let reference = match quoted {
        Some(rest) => rest.split('\'').next()?,
        None => subject
            .strip_prefix("Merge pull request #")?
            .split_once(" from ")?
            .1
            .split_whitespace()
            .next()?,
    };
    let name = reference
        .strip_prefix(BRANCH_PREFIX)
        .or_else(|| reference.split_once('/')?.1.strip_prefix(BRANCH_PREFIX))?;
    (!name.is_empty()).then_some(name)
}

/// The `townhall/*` merges in `git log --numstat --format=MERGE_FORMAT` output.
pub(super) fn parse_merges(out: &str) -> Vec<Merge> {
    out.split(RECORD).filter_map(parse_merge).collect()
}

fn parse_merge(record: &str) -> Option<Merge> {
    let mut lines = record.lines();
    let mut header = lines.next()?.splitn(3, '\t');
    let (parents, date, subject) = (header.next()?, header.next()?, header.next()?);
    merged_branch(subject)?;
    let mut parents = parents.split(' ');
    let (first, second) = (parents.next()?, parents.next()?);
    Some(Merge {
        date: date.to_string(),
        first: first.to_string(),
        second: second.to_string(),
        added: lines.filter_map(added_lines).fold(0, u64::saturating_add),
    })
}

/// Added lines from one numstat line; binary files (`-`) and other lines count 0.
fn added_lines(line: &str) -> Option<u64> {
    line.split('\t').next()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conventional_types_with_scope_and_bang() {
        for subject in [
            "fix: a",
            "fix(auth): a",
            "fix!: a",
            "fix(auth)!: a",
            "Fix: a",
        ] {
            assert_eq!(kind(subject), Kind::Fix, "{subject}");
        }
        for subject in [
            "feat: a",
            "feat(ui): a",
            "feat!: a",
            "feat(ui)!: a",
            "FEAT: a",
        ] {
            assert_eq!(kind(subject), Kind::Feature, "{subject}");
        }
        let others = [
            "chore: a",
            "docs(readme): a",
            "fixup: a",
            "feature: a",
            "Fix the login bug",
            "fix(auth: a",
            " fix: a",
            "fix a: b",
            "",
        ];
        for subject in others {
            assert_eq!(kind(subject), Kind::Other, "{subject}");
        }
    }

    #[test]
    fn merge_subjects_that_name_a_townhall_branch() {
        let cases = [
            ("Merge branch 'townhall/fix-login'", Some("fix-login")),
            ("Merge branch 'townhall/a' into main", Some("a")),
            ("Merge branch 'townhall/a' into 'main'", Some("a")),
            (
                "Merge remote-tracking branch 'origin/townhall/b'",
                Some("b"),
            ),
            ("Merge pull request #12 from owner/townhall/c", Some("c")),
            ("Merge pull request #12 from townhall/d", Some("d")),
            ("Merge branch 'main' into townhall/e", None),
            ("Merge branch 'feature/x'", None),
            ("Merge branch 'townhall/'", None),
            ("Merge pull request #3 from owner/v2/phase-1", None),
            ("Merge pull request #3 from a/b/townhall/f", None),
            ("feat: townhall/g", None),
        ];
        for (subject, name) in cases {
            assert_eq!(merged_branch(subject), name, "{subject}");
        }
    }

    #[test]
    fn records_keep_townhall_merges_and_sum_added_lines() {
        let out = "\u{1e}p1 p2\t2026-03-10\tMerge branch 'townhall/a'\n\n3\t1\tsrc/a.rs\n-\t-\tlogo.png\n10\t0\tdocs/a.md\n\
                   \u{1e}p3 p4\t2026-03-11\tMerge branch 'feature/b'\n\n5\t0\tb.rs\n\
                   \u{1e}p5 p6\t2026-03-12\tMerge pull request #4 from o/townhall/c\tx\n\
                   \u{1e}p7\t2026-03-12\tMerge branch 'townhall/single'\n\
                   \u{1e}garbage";
        let merges = parse_merges(out);
        assert_eq!(
            merges,
            vec![
                Merge {
                    date: "2026-03-10".into(),
                    first: "p1".into(),
                    second: "p2".into(),
                    added: 13,
                },
                Merge {
                    date: "2026-03-12".into(),
                    first: "p5".into(),
                    second: "p6".into(),
                    added: 0,
                },
            ]
        );
    }
}
