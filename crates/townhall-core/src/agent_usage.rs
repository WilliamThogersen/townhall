//! What one agent type used over time: runs and tokens per time bucket, and who used it.
//! A task is one run. Its tokens are the total recorded when it finished, counted at the
//! time it started, since that is the only per-run time the store keeps.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::clock::from_unix_millis;

const HOUR_MS: i64 = 3_600_000;
const DAY_MS: i64 = 24 * HOUR_MS;

pub const TOP_SHARES: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageRange {
    Day,
    Week,
    Month,
}

impl UsageRange {
    pub fn parse(label: &str) -> Option<Self> {
        [Self::Day, Self::Week, Self::Month]
            .into_iter()
            .find(|range| range.label() == label)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Day => "24h",
            Self::Week => "7d",
            Self::Month => "30d",
        }
    }

    fn buckets(self) -> (usize, i64) {
        match self {
            Self::Day => (24, HOUR_MS),
            Self::Week => (28, 6 * HOUR_MS),
            Self::Month => (30, DAY_MS),
        }
    }
}

/// One task as the usage query reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageRun {
    pub name: String,
    pub repo: String,
    pub conversation_id: String,
    pub at_ms: i64,
    pub tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageBucket {
    pub start: String,
    pub runs: u64,
    pub tokens: u64,
}

/// One task, repo or conversation and what it used.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageShare {
    pub key: String,
    pub runs: u64,
    pub tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentUsageSeries {
    pub agent: String,
    pub range: String,
    pub bucket_ms: i64,
    pub since: String,
    pub until: String,
    pub runs: u64,
    pub tokens: u64,
    pub buckets: Vec<UsageBucket>,
    pub by_task: Vec<UsageShare>,
    pub by_repo: Vec<UsageShare>,
    pub by_conversation: Vec<UsageShare>,
}

fn top_shares<'a>(runs: &[&'a UsageRun], key: impl Fn(&'a UsageRun) -> &'a str) -> Vec<UsageShare> {
    let mut totals: BTreeMap<&str, (u64, u64)> = BTreeMap::new();
    for run in runs {
        let entry = totals.entry(key(run)).or_default();
        entry.0 += 1;
        entry.1 += run.tokens;
    }
    let mut shares: Vec<UsageShare> = totals
        .into_iter()
        .map(|(key, (runs, tokens))| UsageShare {
            key: key.to_string(),
            runs,
            tokens,
        })
        .collect();
    shares.sort_by(|a, b| {
        b.tokens
            .cmp(&a.tokens)
            .then(b.runs.cmp(&a.runs))
            .then_with(|| a.key.cmp(&b.key))
    });
    shares.truncate(TOP_SHARES);
    shares
}

/// The runs of `agent` inside the `range` that ends at `now_ms`, in equal buckets, oldest
/// first. A run before the range is left out; one after `now_ms` counts in the last bucket.
pub fn aggregate(
    agent: &str,
    range: UsageRange,
    now_ms: i64,
    runs: &[UsageRun],
) -> AgentUsageSeries {
    let (count, width) = range.buckets();
    let since_ms = now_ms - count as i64 * width;
    let mut buckets: Vec<UsageBucket> = (0..count)
        .map(|i| UsageBucket {
            start: from_unix_millis(since_ms + i as i64 * width),
            runs: 0,
            tokens: 0,
        })
        .collect();
    let inside: Vec<&UsageRun> = runs.iter().filter(|run| run.at_ms >= since_ms).collect();
    for run in &inside {
        let at = usize::try_from((run.at_ms - since_ms) / width).unwrap_or(count - 1);
        let bucket = &mut buckets[at.min(count - 1)];
        bucket.runs += 1;
        bucket.tokens += run.tokens;
    }
    AgentUsageSeries {
        agent: agent.to_string(),
        range: range.label().to_string(),
        bucket_ms: width,
        since: from_unix_millis(since_ms),
        until: from_unix_millis(now_ms),
        runs: inside.len() as u64,
        tokens: inside.iter().map(|run| run.tokens).sum(),
        buckets,
        by_task: top_shares(&inside, |run| run.name.as_str()),
        by_repo: top_shares(&inside, |run| run.repo.as_str()),
        by_conversation: top_shares(&inside, |run| run.conversation_id.as_str()),
    }
}
