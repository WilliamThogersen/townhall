//! A short-lived cache of [`shipped`](super::shipped) per repo, so the home screen can poll
//! stats cheaply. A fresh entry is served without running git. A stale one costs one
//! `git rev-parse`: the same base branch sha reuses its days, a new sha counts again.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use super::{base_sha, shipped_at, ShippedDay};
use crate::Result;

/// How long an entry is served without running git.
pub const FRESH_FOR: Duration = Duration::from_secs(60);
/// How long an unchanged sha keeps its days before they are counted again, since the start
/// of the window moves on even when the branch does not.
pub const REUSE_FOR: Duration = Duration::from_secs(60 * 60);

/// What one cached result was counted for.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheKey {
    pub repo: String,
    pub path: PathBuf,
    pub base_branch: String,
    pub since_days: u32,
}

#[derive(Debug, Clone)]
struct Entry {
    sha: String,
    days: Vec<ShippedDay>,
    checked: Instant,
    counted: Instant,
}

#[derive(Debug)]
pub struct ShippedCache {
    fresh_for: Duration,
    reuse_for: Duration,
    entries: Mutex<HashMap<CacheKey, Entry>>,
}

impl Default for ShippedCache {
    fn default() -> Self {
        Self::new(FRESH_FOR, REUSE_FOR)
    }
}

impl ShippedCache {
    pub fn new(fresh_for: Duration, reuse_for: Duration) -> Self {
        Self {
            fresh_for,
            reuse_for,
            entries: Mutex::new(HashMap::new()),
        }
    }

    /// The days shipped for `key`, from the cache while it is good. On an error (the repo or
    /// its base branch is gone) the entry is dropped and the error returned.
    pub async fn shipped(&self, key: &CacheKey) -> Result<Vec<ShippedDay>> {
        if let Some(days) = self.fresh(key, Instant::now()) {
            return Ok(days);
        }
        let counted = self.count(key).await;
        if counted.is_err() {
            self.lock().remove(key);
        }
        counted
    }

    async fn count(&self, key: &CacheKey) -> Result<Vec<ShippedDay>> {
        let sha = base_sha(&key.path, &key.base_branch).await?;
        if let Some(days) = self.reuse(key, &sha, Instant::now()) {
            return Ok(days);
        }
        let days = shipped_at(&key.path, &sha, key.since_days).await?;
        self.store(key, sha, days.clone(), Instant::now());
        Ok(days)
    }

    fn fresh(&self, key: &CacheKey, now: Instant) -> Option<Vec<ShippedDay>> {
        let entries = self.lock();
        let entry = entries.get(key)?;
        (now.saturating_duration_since(entry.checked) < self.fresh_for).then(|| entry.days.clone())
    }

    /// The days of an entry counted at `sha` not too long ago, now checked again.
    fn reuse(&self, key: &CacheKey, sha: &str, now: Instant) -> Option<Vec<ShippedDay>> {
        let mut entries = self.lock();
        let entry = entries.get(key)?;
        let usable =
            entry.sha == sha && now.saturating_duration_since(entry.counted) < self.reuse_for;
        if !usable {
            return None;
        }
        let checked = Entry {
            checked: now,
            ..entry.clone()
        };
        let days = checked.days.clone();
        entries.insert(key.clone(), checked);
        Some(days)
    }

    fn store(&self, key: &CacheKey, sha: String, days: Vec<ShippedDay>, now: Instant) {
        let entry = Entry {
            sha,
            days,
            checked: now,
            counted: now,
        };
        self.lock().insert(key.clone(), entry);
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<CacheKey, Entry>> {
        self.entries.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secs(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    fn key() -> CacheKey {
        CacheKey {
            repo: "shop".into(),
            path: PathBuf::from("/work/shop"),
            base_branch: "main".into(),
            since_days: 730,
        }
    }

    fn days(agents: u32) -> Vec<ShippedDay> {
        vec![ShippedDay {
            date: "2026-11-03".into(),
            agents,
            ..ShippedDay::default()
        }]
    }

    #[test]
    fn an_entry_is_fresh_for_the_ttl_only() {
        let cache = ShippedCache::new(secs(60), secs(3_600));
        let t0 = Instant::now();
        assert_eq!(cache.fresh(&key(), t0), None);
        cache.store(&key(), "a".into(), days(1), t0);
        assert_eq!(cache.fresh(&key(), t0 + secs(59)), Some(days(1)));
        assert_eq!(cache.fresh(&key(), t0 + secs(60)), None);
        let other = CacheKey {
            base_branch: "develop".into(),
            ..key()
        };
        assert_eq!(
            cache.fresh(&other, t0),
            None,
            "the base branch is part of the key"
        );
    }

    #[test]
    fn a_stale_entry_is_reused_only_for_the_same_sha_and_not_forever() {
        let cache = ShippedCache::new(secs(60), secs(3_600));
        let t0 = Instant::now();
        assert_eq!(cache.reuse(&key(), "a", t0), None);
        cache.store(&key(), "a".into(), days(2), t0);
        assert_eq!(cache.reuse(&key(), "b", t0 + secs(90)), None);
        assert_eq!(cache.reuse(&key(), "a", t0 + secs(90)), Some(days(2)));
        assert_eq!(
            cache.fresh(&key(), t0 + secs(140)),
            Some(days(2)),
            "reusing checks it again"
        );
        assert_eq!(cache.reuse(&key(), "a", t0 + secs(3_600)), None);
    }
}
