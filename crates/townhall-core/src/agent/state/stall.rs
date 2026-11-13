use std::collections::HashSet;
use std::sync::{LazyLock, Mutex};

pub const STALL_ENV: &str = "TOWNHALL_STALL_SECS";
pub const DEFAULT_STALL_SECS: u64 = 300;
pub const API_SETTLE_MS: i64 = 5_000;
pub const NUDGE: &str = "continue";

const MAX_NUDGED: usize = 512;
const SCREEN_LINES: usize = 8;
const HANG_PHRASES: [&str; 4] = [
    "went to sleep",
    "connection lost",
    "api error",
    "connection error",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Hang {
    Stalled,
    ApiError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Verdict {
    Fine,
    Nudge,
    Surface,
}

pub fn limit_secs() -> u64 {
    std::env::var(STALL_ENV)
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(DEFAULT_STALL_SECS)
}

pub(super) fn stalled(now: u64, activity: u64, written: Option<u64>, limit: u64) -> bool {
    let last = activity.max(written.unwrap_or(0));
    now.saturating_sub(last) >= limit
}

pub(super) fn hang_text(text: &str) -> bool {
    let lower = text.to_lowercase();
    HANG_PHRASES.iter().any(|p| lower.contains(p))
}

pub(super) fn hang_on_screen(screen: &str) -> bool {
    let lines: Vec<&str> = screen.lines().filter(|l| !l.trim().is_empty()).collect();
    let from = lines.len().saturating_sub(SCREEN_LINES);
    hang_text(&lines[from..].join("\n"))
}

pub(super) fn verdict(hang: Option<Hang>, nudged: bool) -> Verdict {
    match (hang, nudged) {
        (None, _) => Verdict::Fine,
        (Some(_), false) => Verdict::Nudge,
        (Some(_), true) => Verdict::Surface,
    }
}

pub(super) fn reason(hang: Hang, limit: u64) -> String {
    match hang {
        Hang::Stalled => format!(
            "Stalled: no transcript or screen activity for {} min. It was nudged once with `{NUDGE}` and is still silent",
            limit.div_ceil(60)
        ),
        Hang::ApiError => format!(
            "Stopped on an API or connection error and did not recover after one automatic `{NUDGE}`"
        ),
    }
}

static NUDGED: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(Mutex::default);

pub(super) fn session_id(pane_id: &str, created: u64) -> String {
    format!("{pane_id}@{created}")
}

pub(super) fn was_nudged(session: &str) -> bool {
    NUDGED
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .contains(session)
}

pub(super) fn mark_nudged(session: &str) {
    let mut set = NUDGED.lock().unwrap_or_else(|p| p.into_inner());
    if set.len() >= MAX_NUDGED {
        set.clear();
    }
    set.insert(session.to_string());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pane_and_transcript_quiet_for_the_limit_is_stalled() {
        assert!(!stalled(1_000, 800, Some(900), 300));
        assert!(!stalled(1_000, 900, Some(700), 300));
        assert!(stalled(1_000, 700, Some(700), 300));
        assert!(stalled(1_000, 700, None, 300));
        assert!(!stalled(1_000, 1_005, None, 300));
    }

    #[test]
    fn a_hang_is_nudged_once_then_surfaced() {
        assert_eq!(verdict(None, false), Verdict::Fine);
        assert_eq!(verdict(None, true), Verdict::Fine);
        assert_eq!(verdict(Some(Hang::Stalled), false), Verdict::Nudge);
        assert_eq!(verdict(Some(Hang::Stalled), true), Verdict::Surface);
        assert_eq!(verdict(Some(Hang::ApiError), false), Verdict::Nudge);
        assert_eq!(verdict(Some(Hang::ApiError), true), Verdict::Surface);
    }

    #[test]
    fn api_errors_sleep_and_lost_connections_are_hangs() {
        assert!(hang_text("API Error: 529 Overloaded"));
        assert!(hang_text("The session went to sleep"));
        assert!(hang_text("Connection lost, retrying"));
        assert!(!hang_text("All done, tests pass"));
        assert!(hang_on_screen("old\n\n\nConnection lost\n"));
        let long = format!("API Error\n{}", "line\n".repeat(20));
        assert!(!hang_on_screen(&long));
    }

    #[test]
    fn the_reason_names_the_cause_and_the_wait() {
        assert!(reason(Hang::Stalled, 300).contains("5 min"));
        assert!(reason(Hang::ApiError, 300).contains("API"));
    }

    #[test]
    fn a_session_is_nudged_once() {
        let id = session_id("%stall-test", 1);
        assert!(!was_nudged(&id));
        mark_nudged(&id);
        assert!(was_nudged(&id));
        assert!(!was_nudged(&session_id("%stall-test", 2)));
    }

    #[test]
    fn the_limit_comes_from_the_environment_or_defaults() {
        assert_eq!(limit_secs(), DEFAULT_STALL_SECS);
    }
}
