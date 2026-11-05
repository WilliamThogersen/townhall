//! Polls Claude plan usage, keeps the last good reading and backs off after a 429.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use serde_json::Value;

use super::{
    parse_usage, push_reading, session_rate, usage_error, weekly_rate, ClaudeUsage, Reading,
    UsageWindow,
};
use crate::fsutil::write_atomic;
use crate::redact::redact;

pub const POLL_EVERY: Duration = Duration::from_secs(5 * 60);
pub const BACKOFF_AFTER_429: Duration = Duration::from_secs(15 * 60);
pub const REUSE_FOR: Duration = Duration::from_secs(60);
pub const CACHED_ERROR: &str = "Not refreshed yet";
const SIGN_IN: &str = "Sign in to Claude Code to see usage";
const OFFLINE: &str = "Usage unavailable (offline?)";
const UNREADABLE: &str = "Usage unavailable (unreadable response)";
const TARGET: &str = "townhall::usage";

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpReply {
    pub status: u16,
    pub body: String,
}

/// Where the token and the HTTP answer come from; tests inject their own.
pub trait UsageSource: Send + Sync + 'static {
    fn token(&self) -> BoxFuture<'_, Option<String>>;
    fn get<'a>(&'a self, token: &'a str) -> BoxFuture<'a, Result<HttpReply, String>>;
}

/// Unix milliseconds.
pub type Clock = Arc<dyn Fn() -> i64 + Send + Sync>;

#[derive(Debug, Default)]
struct State {
    last: Option<ClaudeUsage>,
    history: Vec<Reading>,
    attempted_at: Option<i64>,
    backoff_until: i64,
    next_delay: Option<Duration>,
}

/// A failed fetch: what the reader sees, and what the log gets.
struct Failure {
    error: String,
    status: Option<u16>,
    cause: Option<String>,
}

impl Failure {
    fn new(error: &str) -> Self {
        Self {
            error: error.to_string(),
            status: None,
            cause: None,
        }
    }
}

pub struct UsageService {
    source: Box<dyn UsageSource>,
    clock: Clock,
    cache: Option<PathBuf>,
    gate: tokio::sync::Mutex<()>,
    state: Mutex<State>,
    polling: AtomicBool,
}

impl UsageService {
    /// `cache` keeps the last good reading across restarts; it is shown, marked stale
    /// (`CACHED_ERROR`), until a fetch succeeds.
    pub fn new(source: impl UsageSource, clock: Clock, cache: Option<PathBuf>) -> Self {
        let cached = cache
            .as_deref()
            .and_then(load_cache)
            .map(|usage| ClaudeUsage {
                error: Some(CACHED_ERROR.to_string()),
                ..usage
            });
        Self {
            source: Box::new(source),
            clock,
            cache,
            gate: tokio::sync::Mutex::new(()),
            state: Mutex::new(State {
                last: cached,
                ..State::default()
            }),
            polling: AtomicBool::new(false),
        }
    }

    pub fn last(&self) -> Option<ClaudeUsage> {
        self.state().last.clone()
    }

    /// How long to wait before the next poll: five minutes, fifteen after a 429.
    pub fn poll_delay(&self) -> Duration {
        self.state().next_delay.unwrap_or(POLL_EVERY)
    }

    /// The last reading at once, or the first fetch when there is none. Starts polling on the
    /// first call, so a daemon nobody reads usage from never touches the Keychain or the network.
    pub async fn read(self: &Arc<Self>) -> ClaudeUsage {
        self.ensure_polling();
        match self.last() {
            Some(last) => last,
            None => self.refresh(false).await,
        }
    }

    /// Fetches now. Unless `force`, an attempt under a minute old, or a 429 back-off still
    /// running, answers with the last reading instead. Concurrent callers share one fetch.
    pub async fn refresh(&self, force: bool) -> ClaudeUsage {
        let _turn = self.gate.lock().await;
        if !force {
            if let Some(last) = self.reusable() {
                return last;
            }
        }
        self.fetch().await
    }

    fn ensure_polling(self: &Arc<Self>) {
        if self.polling.swap(true, Ordering::SeqCst) {
            return;
        }
        let weak = Arc::downgrade(self);
        tokio::spawn(async move {
            let mut force = false;
            loop {
                let Some(service) = weak.upgrade() else {
                    return;
                };
                service.refresh(force).await;
                let delay = service.poll_delay();
                drop(service);
                tokio::time::sleep(delay).await;
                force = true;
            }
        });
    }

    fn reusable(&self) -> Option<ClaudeUsage> {
        let now = self.now();
        let state = self.state();
        let attempted = state.attempted_at?;
        let waiting = now - attempted < millis(REUSE_FOR) || now < state.backoff_until;
        state.last.clone().filter(|_| waiting)
    }

    async fn fetch(&self) -> ClaudeUsage {
        let started = Instant::now();
        let now = self.now();
        let (fetched, status) = match self.source.token().await {
            None => (Err(Failure::new(SIGN_IN)), None),
            Some(token) => match self.source.get(&token).await {
                Err(cause) => {
                    let failure = Failure {
                        cause: Some(cause),
                        ..Failure::new(OFFLINE)
                    };
                    (Err(failure), None)
                }
                Ok(reply) => (read_reply(&reply, now), Some(reply.status)),
            },
        };
        let next = self.record(fetched, status, now);
        if next.error.is_none() {
            tracing::debug!(
                target: TARGET, event = "usage.fetched", status = status.unwrap_or(0),
                duration_ms = started.elapsed().as_millis() as u64, "usage fetched"
            );
            self.save(&next).await;
        }
        next
    }

    fn record(
        &self,
        fetched: Result<ClaudeUsage, Failure>,
        status: Option<u16>,
        now: i64,
    ) -> ClaudeUsage {
        let mut state = self.state();
        let before = state.last.as_ref().and_then(|u| u.error.clone());
        let backing_off = status == Some(429);
        let next = match fetched {
            Ok(usage) => with_rates(&mut state.history, usage),
            Err(failure) => {
                log_failure(&failure, before.as_deref());
                failed(failure.error, state.last.as_ref(), now)
            }
        };
        let delay = if backing_off {
            BACKOFF_AFTER_429
        } else {
            POLL_EVERY
        };
        state.attempted_at = Some(now);
        state.backoff_until = if backing_off { now + millis(delay) } else { 0 };
        state.next_delay = Some(delay);
        state.last = Some(next.clone());
        next
    }

    async fn save(&self, usage: &ClaudeUsage) {
        let Some(path) = self.cache.clone() else {
            return;
        };
        let written = match serde_json::to_vec(usage) {
            Ok(bytes) => tokio::task::spawn_blocking(move || write_atomic(&path, &bytes, false))
                .await
                .map_err(|e| e.to_string())
                .and_then(|done| done.map_err(|e| e.to_string())),
            Err(e) => Err(e.to_string()),
        };
        if let Err(e) = written {
            tracing::warn!(
                target: TARGET, event = "usage.cache_not_saved",
                error = %redact(&e), "cannot save the usage cache"
            );
        }
    }

    fn now(&self) -> i64 {
        (self.clock)()
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn millis(d: Duration) -> i64 {
    i64::try_from(d.as_millis()).unwrap_or(i64::MAX)
}

fn read_reply(reply: &HttpReply, now: i64) -> Result<ClaudeUsage, Failure> {
    if !(200..300).contains(&reply.status) {
        let failure = Failure::new(&usage_error(reply.status));
        return Err(Failure {
            status: Some(reply.status),
            ..failure
        });
    }
    match serde_json::from_str::<Value>(&reply.body) {
        Ok(body) => Ok(parse_usage(&body, now)),
        Err(e) => Err(Failure {
            status: Some(reply.status),
            cause: Some(e.to_string()),
            ..Failure::new(UNREADABLE)
        }),
    }
}

/// The last numbers and the time they were read, so "updated" never shows a failed attempt.
fn failed(error: String, last: Option<&ClaudeUsage>, now: i64) -> ClaudeUsage {
    ClaudeUsage {
        session: last.and_then(|u| u.session.clone()),
        weekly: last.and_then(|u| u.weekly.clone()),
        updated_at: last.map_or(now, |u| u.updated_at),
        error: Some(error),
    }
}

fn with_rates(history: &mut Vec<Reading>, usage: ClaudeUsage) -> ClaudeUsage {
    let now = usage.updated_at;
    match &usage.session {
        Some(s) => {
            let reading = Reading {
                at: now,
                used_percent: s.used_percent,
                resets_at: s.resets_at,
            };
            *history = push_reading(history, reading);
        }
        None => history.clear(),
    }
    let rated = |w: UsageWindow, rate: Option<f64>| UsageWindow {
        rate_per_hour: rate,
        ..w
    };
    ClaudeUsage {
        session: usage.session.map(|w| {
            let rate = session_rate(&w, history, now);
            rated(w, rate)
        }),
        weekly: usage.weekly.map(|w| {
            let rate = weekly_rate(&w, now);
            rated(w, rate)
        }),
        ..usage
    }
}

/// Logged once per change, so polling does not repeat the same failure every five minutes.
fn log_failure(failure: &Failure, before: Option<&str>) {
    if before == Some(failure.error.as_str()) {
        return;
    }
    let detail = redact(failure.cause.as_deref().unwrap_or(&failure.error));
    match failure.status {
        Some(status) => tracing::warn!(
            target: TARGET, event = "usage.fetch_failed", status, error = %detail,
            "usage fetch failed"
        ),
        None => tracing::warn!(
            target: TARGET, event = "usage.fetch_failed", error = %detail, "usage fetch failed"
        ),
    }
}

fn load_cache(path: &Path) -> Option<ClaudeUsage> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => return ignored_cache(&e.to_string()),
    };
    match serde_json::from_str::<ClaudeUsage>(&text) {
        Ok(usage) if usage.session.is_some() || usage.weekly.is_some() => Some(usage),
        Ok(_) => ignored_cache("no windows"),
        Err(e) => ignored_cache(&e.to_string()),
    }
}

fn ignored_cache(why: &str) -> Option<ClaudeUsage> {
    tracing::warn!(
        target: TARGET, event = "usage.cache_ignored", error = %redact(why),
        "usage cache ignored"
    );
    None
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};

    use super::*;

    const T0: i64 = 1_793_800_000_000;
    const MIN: i64 = 60_000;
    const SECRET: &str = "sk-ant-oat01-secret-token";

    type Answer = Result<HttpReply, String>;

    #[derive(Clone, Default)]
    struct Fake {
        token: Arc<Mutex<Option<String>>>,
        answers: Arc<Mutex<VecDeque<Answer>>>,
        calls: Arc<AtomicUsize>,
        seen_tokens: Arc<Mutex<Vec<String>>>,
    }

    impl Fake {
        fn signed_in() -> Self {
            let fake = Self::default();
            *fake.token.lock().unwrap() = Some(SECRET.to_string());
            fake
        }

        fn answer(&self, answer: Answer) -> &Self {
            self.answers.lock().unwrap().push_back(answer);
            self
        }

        fn calls(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
    }

    impl UsageSource for Fake {
        fn token(&self) -> BoxFuture<'_, Option<String>> {
            Box::pin(async { self.token.lock().unwrap().clone() })
        }

        fn get<'a>(&'a self, token: &'a str) -> BoxFuture<'a, Answer> {
            Box::pin(async move {
                self.calls.fetch_add(1, Ordering::SeqCst);
                self.seen_tokens.lock().unwrap().push(token.to_string());
                self.answers
                    .lock()
                    .unwrap()
                    .pop_front()
                    .expect("an unexpected fetch")
            })
        }
    }

    fn ok(session: f64, resets_at: i64) -> Answer {
        let body = serde_json::json!({
            "five_hour": { "utilization": session, "resets_at": resets_at },
            "seven_day": { "utilization": 54.0, "resets_at": T0 + 5 * 24 * 60 * MIN },
        });
        Ok(HttpReply {
            status: 200,
            body: body.to_string(),
        })
    }

    fn status(status: u16) -> Answer {
        Ok(HttpReply {
            status,
            body: "{}".into(),
        })
    }

    struct Rig {
        fake: Fake,
        clock: Arc<AtomicI64>,
        service: UsageService,
    }

    fn rig_with(fake: Fake, cache: Option<PathBuf>) -> Rig {
        let clock = Arc::new(AtomicI64::new(T0));
        let now = clock.clone();
        let service = UsageService::new(
            fake.clone(),
            Arc::new(move || now.load(Ordering::SeqCst)),
            cache,
        );
        Rig {
            fake,
            clock,
            service,
        }
    }

    fn rig() -> Rig {
        rig_with(Fake::signed_in(), None)
    }

    impl Rig {
        fn advance(&self, ms: i64) {
            self.clock.fetch_add(ms, Ordering::SeqCst);
        }
    }

    #[tokio::test]
    async fn a_fetch_returns_both_windows_with_their_rates() {
        let r = rig();
        r.fake.answer(ok(30.0, T0 + 150 * MIN));
        let usage = r.service.refresh(false).await;
        let session = usage.session.unwrap();
        assert_eq!(session.used_percent, 30.0);
        assert_eq!(session.rate_per_hour, Some(12.0));
        assert_eq!(usage.weekly.unwrap().rate_per_hour, Some(1.125));
        assert_eq!((usage.updated_at, usage.error), (T0, None));
        assert_eq!(*r.fake.seen_tokens.lock().unwrap(), [SECRET]);
    }

    #[tokio::test]
    async fn the_token_is_never_in_what_is_returned() {
        let r = rig();
        r.fake.answer(ok(30.0, T0 + 150 * MIN));
        let usage = r.service.refresh(false).await;
        assert!(!serde_json::to_string(&usage).unwrap().contains("secret"));
        let leaky = Err(format!("connect failed with Bearer {SECRET}"));
        r.fake.answer(leaky);
        r.advance(10 * MIN);
        let usage = r.service.refresh(false).await;
        assert!(!serde_json::to_string(&usage).unwrap().contains("secret"));
    }

    #[tokio::test]
    async fn without_a_token_nothing_is_fetched() {
        let r = rig_with(Fake::default(), None);
        let usage = r.service.refresh(false).await;
        assert_eq!(r.fake.calls(), 0);
        assert_eq!(
            usage.error.as_deref(),
            Some("Sign in to Claude Code to see usage")
        );
        assert_eq!(
            (usage.session, usage.weekly, usage.updated_at),
            (None, None, T0)
        );
    }

    #[tokio::test]
    async fn a_failed_refresh_keeps_the_last_numbers_and_their_time() {
        let r = rig();
        r.fake.answer(ok(30.0, T0 + 150 * MIN));
        let good = r.service.refresh(false).await;
        let cases = [
            (status(500), "Usage unavailable (HTTP 500)"),
            (status(401), "Sign in to Claude Code to see usage"),
            (Err("timed out".into()), "Usage unavailable (offline?)"),
            (
                Ok(HttpReply {
                    status: 200,
                    body: "<html>".into(),
                }),
                "Usage unavailable (unreadable response)",
            ),
        ];
        for (answer, error) in cases {
            r.advance(2 * MIN);
            r.fake.answer(answer);
            let usage = r.service.refresh(false).await;
            assert_eq!(usage.error.as_deref(), Some(error));
            assert_eq!(usage.updated_at, good.updated_at);
            assert_eq!(
                (&usage.session, &usage.weekly),
                (&good.session, &good.weekly)
            );
        }
    }

    #[tokio::test]
    async fn the_next_success_clears_the_error() {
        let r = rig();
        r.fake.answer(status(500)).answer(ok(31.0, T0 + 150 * MIN));
        r.service.refresh(false).await;
        r.advance(2 * MIN);
        let usage = r.service.refresh(false).await;
        assert_eq!(usage.error, None);
        assert_eq!(usage.session.unwrap().used_percent, 31.0);
        assert_eq!(usage.updated_at, T0 + 2 * MIN);
    }

    #[tokio::test]
    async fn readings_inside_a_minute_reuse_the_last_result_unless_forced() {
        let r = rig();
        r.fake.answer(ok(30.0, T0 + 150 * MIN));
        let first = r.service.refresh(false).await;
        r.advance(59_000);
        assert_eq!(r.service.refresh(false).await, first);
        assert_eq!(r.fake.calls(), 1);
        r.fake.answer(ok(31.0, T0 + 150 * MIN));
        r.service.refresh(true).await;
        assert_eq!(r.fake.calls(), 2);
        r.advance(61_000);
        r.fake.answer(ok(32.0, T0 + 150 * MIN));
        let third = r.service.refresh(false).await;
        assert_eq!(
            (r.fake.calls(), third.session.unwrap().used_percent),
            (3, 32.0)
        );
    }

    #[tokio::test]
    async fn a_failure_is_not_retried_inside_the_minute_either() {
        let r = rig();
        r.fake.answer(status(500));
        r.service.refresh(false).await;
        r.advance(30_000);
        r.service.refresh(false).await;
        assert_eq!(r.fake.calls(), 1);
    }

    #[tokio::test]
    async fn two_readers_at_once_share_one_fetch() {
        let r = rig();
        r.fake.answer(ok(30.0, T0 + 150 * MIN));
        let (a, b) = tokio::join!(r.service.refresh(false), r.service.refresh(false));
        assert_eq!((r.fake.calls(), a), (1, b));
    }

    #[tokio::test]
    async fn a_429_backs_off_for_fifteen_minutes() {
        let r = rig();
        assert_eq!(r.service.poll_delay(), POLL_EVERY);
        r.fake.answer(status(429));
        let usage = r.service.refresh(false).await;
        assert_eq!(
            usage.error.as_deref(),
            Some("Usage rate limited, retrying later")
        );
        assert_eq!(r.service.poll_delay(), BACKOFF_AFTER_429);
        r.advance(10 * MIN);
        r.service.refresh(false).await;
        assert_eq!(r.fake.calls(), 1, "reads inside the back-off wait");
        r.advance(6 * MIN);
        r.fake.answer(ok(30.0, T0 + 150 * MIN));
        let usage = r.service.refresh(false).await;
        assert_eq!((r.fake.calls(), usage.error), (2, None));
        assert_eq!(r.service.poll_delay(), POLL_EVERY);
    }

    #[tokio::test]
    async fn the_session_rate_follows_the_last_half_hour() {
        let r = rig();
        let resets = T0 + 150 * MIN;
        r.fake.answer(ok(20.0, resets)).answer(ok(30.0, resets));
        r.service.refresh(false).await;
        r.advance(30 * MIN);
        let usage = r.service.refresh(false).await;
        assert_eq!(usage.session.unwrap().rate_per_hour, Some(20.0));
    }

    #[tokio::test]
    async fn a_good_reading_is_cached_and_shown_stale_after_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("usage.json");
        let r = rig_with(Fake::signed_in(), Some(path.clone()));
        r.fake.answer(ok(30.0, T0 + 150 * MIN));
        let good = r.service.refresh(false).await;
        assert!(!std::fs::read_to_string(&path).unwrap().contains("secret"));

        let again = rig_with(Fake::signed_in(), Some(path.clone()));
        again.fake.answer(status(500));
        let usage = again.service.refresh(false).await;
        assert_eq!(usage.error.as_deref(), Some("Usage unavailable (HTTP 500)"));
        assert_eq!(
            (usage.session, usage.updated_at),
            (good.session, good.updated_at)
        );
        let cached = rig_with(Fake::signed_in(), Some(path))
            .service
            .last()
            .unwrap();
        assert_eq!(cached.error.as_deref(), Some(CACHED_ERROR));
        assert_eq!(cached.weekly, good.weekly);
    }

    #[tokio::test]
    async fn a_failed_fetch_leaves_the_cache_alone() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("usage.json");
        let r = rig_with(Fake::signed_in(), Some(path.clone()));
        r.fake.answer(ok(30.0, T0 + 150 * MIN)).answer(status(500));
        r.service.refresh(false).await;
        let before = std::fs::read_to_string(&path).unwrap();
        r.advance(2 * MIN);
        r.service.refresh(false).await;
        assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
    }

    #[test]
    fn a_cache_that_is_not_a_reading_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("usage.json");
        for text in [
            "",
            "garbage",
            "{}",
            r#"{"session":null,"weekly":null,"updatedAt":5,"error":null}"#,
            r#"{"session":{"usedPercent":"high"},"updatedAt":5}"#,
        ] {
            std::fs::write(&path, text).unwrap();
            assert_eq!(
                rig_with(Fake::default(), Some(path.clone())).service.last(),
                None,
                "{text}"
            );
        }
    }

    #[tokio::test]
    async fn the_first_read_fetches_and_later_reads_get_the_last_reading() {
        let r = rig();
        let service = Arc::new(r.service);
        r.fake.answer(ok(30.0, T0 + 150 * MIN));
        let first = service.read().await;
        assert_eq!(first.session.unwrap().used_percent, 30.0);
        let second = service.read().await;
        assert_eq!((r.fake.calls(), second.updated_at), (1, T0));
    }
}
