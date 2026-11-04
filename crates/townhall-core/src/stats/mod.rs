//! Shipped stats: what reached a repo's base branch from `townhall/*` branches.

mod log;

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::cmd::run;
use crate::{Error, Result};
use log::{kind, parse_merges, Kind, Merge, MERGE_FORMAT};

/// What shipped on one day.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShippedDay {
    /// `YYYY-MM-DD`.
    pub date: String,
    pub fixes: u32,
    pub features: u32,
    pub lines: u64,
    pub agents: u32,
}

/// Days with `townhall/*` merges into `base_branch` in the last `since_days`, ascending.
/// A merge counts on its committer's local date; its commits are `merge^1..merge^2`.
pub async fn shipped(repo: &Path, base_branch: &str, since_days: u32) -> Result<Vec<ShippedDay>> {
    check_branch_name(base_branch)?;
    check_repo(repo).await?;
    check_branch(repo, base_branch).await?;
    let merges = list_merges(repo, base_branch, since_days).await?;
    let mut days = Vec::with_capacity(merges.len());
    for merge in &merges {
        days.push(ship(repo, merge).await?);
    }
    Ok(merge_days(vec![days]))
}

/// Sums same-date entries across repos, ascending by date.
pub fn merge_days(days: Vec<Vec<ShippedDay>>) -> Vec<ShippedDay> {
    days.into_iter()
        .flatten()
        .fold(BTreeMap::<String, ShippedDay>::new(), |mut by_date, day| {
            let total = match by_date.remove(&day.date) {
                Some(earlier) => add(earlier, day),
                None => day,
            };
            by_date.insert(total.date.clone(), total);
            by_date
        })
        .into_values()
        .collect()
}

fn add(a: ShippedDay, b: ShippedDay) -> ShippedDay {
    ShippedDay {
        fixes: a.fixes.saturating_add(b.fixes),
        features: a.features.saturating_add(b.features),
        lines: a.lines.saturating_add(b.lines),
        agents: a.agents.saturating_add(b.agents),
        date: a.date,
    }
}

fn check_branch_name(base: &str) -> Result<()> {
    let bad = base.is_empty()
        || base.starts_with('-')
        || base.chars().any(|c| c.is_whitespace() || c.is_control());
    if bad {
        return Err(Error::InvalidInput(format!(
            "base branch `{base}` is not a branch name"
        )));
    }
    Ok(())
}

async fn check_repo(repo: &Path) -> Result<()> {
    if !repo.is_dir() {
        let shown = repo.display();
        return Err(Error::InvalidInput(format!("repo `{shown}` not found")));
    }
    match run("git", &["rev-parse", "--git-dir"], Some(repo)).await {
        Err(Error::Command { .. }) => Err(Error::InvalidInput(format!(
            "`{}` is not a git repository",
            repo.display()
        ))),
        other => other.map(drop),
    }
}

async fn check_branch(repo: &Path, base: &str) -> Result<()> {
    let rev = format!("{base}^{{commit}}");
    let args = ["rev-parse", "--verify", "--quiet", &rev];
    match run("git", &args, Some(repo)).await {
        Err(Error::Command { .. }) => Err(Error::InvalidInput(format!(
            "branch `{base}` not found in repo `{}`",
            repo.display()
        ))),
        other => other.map(drop),
    }
}

/// `townhall/*` merges on the first-parent line of `base`, with lines added against `merge^1`.
async fn list_merges(repo: &Path, base: &str, since_days: u32) -> Result<Vec<Merge>> {
    let format = format!("--format={MERGE_FORMAT}");
    let since = format!("--since={since_days} days ago");
    let args = [
        "log",
        "--first-parent",
        "--merges",
        "--diff-merges=first-parent",
        "--numstat",
        "--no-color",
        "--date=format:%Y-%m-%d",
        &format,
        &since,
        base,
        "--",
    ];
    let out = run("git", &args, Some(repo)).await?;
    Ok(parse_merges(&out))
}

/// Counts the conventional types of the commits one merge brought in.
async fn ship(repo: &Path, merge: &Merge) -> Result<ShippedDay> {
    let range = format!("{}..{}", merge.first, merge.second);
    let out = run("git", &["log", "--format=%s", &range, "--"], Some(repo)).await?;
    let kinds: Vec<Kind> = out.lines().map(kind).collect();
    let count = |wanted: Kind| {
        let n = kinds.iter().filter(|k| **k == wanted).count();
        u32::try_from(n).unwrap_or(u32::MAX)
    };
    Ok(ShippedDay {
        date: merge.date.clone(),
        fixes: count(Kind::Fix),
        features: count(Kind::Feature),
        lines: merge.added,
        agents: 1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(date: &str, fixes: u32, features: u32, lines: u64, agents: u32) -> ShippedDay {
        ShippedDay {
            date: date.to_string(),
            fixes,
            features,
            lines,
            agents,
        }
    }

    #[test]
    fn merge_days_sums_same_dates_across_repos() {
        let shop = vec![
            day("2026-03-10", 1, 0, 10, 1),
            day("2026-03-12", 0, 2, 5, 2),
        ];
        let site = vec![day("2026-03-09", 3, 1, 7, 1), day("2026-03-12", 1, 1, 1, 1)];
        let merged = merge_days(vec![shop, vec![], site]);
        assert_eq!(
            merged,
            vec![
                day("2026-03-09", 3, 1, 7, 1),
                day("2026-03-10", 1, 0, 10, 1),
                day("2026-03-12", 1, 3, 6, 3),
            ]
        );
        assert!(merge_days(vec![]).is_empty());
    }

    #[test]
    fn merge_days_saturates_instead_of_overflowing() {
        let big = day("2026-01-01", u32::MAX, 0, u64::MAX, 1);
        let merged = merge_days(vec![vec![big.clone()], vec![day("2026-01-01", 1, 0, 1, 1)]]);
        assert_eq!(merged, vec![ShippedDay { agents: 2, ..big }]);
    }

    #[test]
    fn days_serialize_with_the_contract_keys() {
        let json = serde_json::to_value(day("2026-03-10", 1, 2, 3, 4)).unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "date": "2026-03-10", "fixes": 1, "features": 2, "lines": 3, "agents": 4 })
        );
    }
}
