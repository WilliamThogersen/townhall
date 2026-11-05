//! How fast a window is filling: percentage points per hour.

use super::UsageWindow;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reading {
    pub at: i64,
    pub used_percent: f64,
    pub resets_at: Option<i64>,
}

const HOUR_MS: i64 = 3_600_000;
pub const SESSION_SPAN_MS: i64 = 5 * HOUR_MS;
pub const WEEK_SPAN_MS: i64 = 7 * 24 * HOUR_MS;
const RECENT_RATE_MS: i64 = 30 * 60_000;
const MIN_RATE_SPAN_MS: i64 = 10 * 60_000;
const HISTORY_MS: i64 = HOUR_MS;
const RESET_JITTER_MS: i64 = 60_000;

/// Points per hour since the window started (its reset minus its length); `None` when the
/// reset is unknown or the window has not started.
pub fn window_average_rate(window: &UsageWindow, span_ms: i64, now: i64) -> Option<f64> {
    let elapsed = now - (window.resets_at? - span_ms);
    (elapsed > 0).then(|| window.used_percent / (elapsed as f64 / HOUR_MS as f64))
}

/// Appends a reading and drops what is over an hour old or from before a reset (the reset
/// time moved, or usage fell).
pub fn push_reading(history: &[Reading], reading: Reading) -> Vec<Reading> {
    let reset = history.last().is_some_and(|prev| {
        let moved = (reading.resets_at.unwrap_or(0) - prev.resets_at.unwrap_or(0)).abs();
        moved > RESET_JITTER_MS || reading.used_percent < prev.used_percent
    });
    let kept = if reset { &[] } else { history };
    kept.iter()
        .chain([&reading])
        .filter(|r| reading.at - r.at <= HISTORY_MS)
        .copied()
        .collect()
}

/// The rate over the last half hour; `None` with fewer than two readings spanning ten minutes.
pub fn recent_rate(history: &[Reading], now: i64) -> Option<f64> {
    let mut recent = history.iter().filter(|r| now - r.at <= RECENT_RATE_MS);
    let first = recent.next()?;
    let last = recent.next_back()?;
    let span = last.at - first.at;
    (span >= MIN_RATE_SPAN_MS)
        .then(|| (last.used_percent - first.used_percent) / (span as f64 / HOUR_MS as f64))
}

/// Recent readings, else the window average.
pub fn session_rate(window: &UsageWindow, history: &[Reading], now: i64) -> Option<f64> {
    recent_rate(history, now).or_else(|| window_average_rate(window, SESSION_SPAN_MS, now))
}

pub fn weekly_rate(window: &UsageWindow, now: i64) -> Option<f64> {
    window_average_rate(window, WEEK_SPAN_MS, now)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: i64 = 3_600_000;
    const MIN: i64 = 60_000;
    const NOW: i64 = 1_793_800_000_000;

    fn window(used: f64, resets_in: Option<i64>) -> UsageWindow {
        UsageWindow {
            used_percent: used,
            resets_at: resets_in.map(|d| NOW + d),
            rate_per_hour: None,
        }
    }

    fn reading(ago: i64, used: f64, resets_in: i64) -> Reading {
        Reading {
            at: NOW - ago,
            used_percent: used,
            resets_at: Some(NOW + resets_in),
        }
    }

    #[test]
    fn the_session_average_runs_from_the_start_of_the_window() {
        let w = window(30.0, Some(2 * HOUR + 30 * MIN));
        assert_eq!(session_rate(&w, &[], NOW), Some(12.0));
    }

    #[test]
    fn the_weekly_rate_is_the_average_since_the_week_began() {
        let w = window(54.0, Some(5 * 24 * HOUR));
        assert_eq!(weekly_rate(&w, NOW), Some(1.125));
    }

    #[test]
    fn no_rate_without_a_reset_time_or_before_the_window_started() {
        assert_eq!(weekly_rate(&window(10.0, None), NOW), None);
        assert_eq!(session_rate(&window(10.0, Some(5 * HOUR)), &[], NOW), None);
        assert_eq!(session_rate(&window(10.0, Some(6 * HOUR)), &[], NOW), None);
    }

    #[test]
    fn recent_readings_beat_the_average() {
        let w = window(30.0, Some(2 * HOUR));
        let history = [
            reading(30 * MIN, 20.0, 2 * HOUR),
            reading(0, 30.0, 2 * HOUR),
        ];
        assert_eq!(recent_rate(&history, NOW), Some(20.0));
        assert_eq!(session_rate(&w, &history, NOW), Some(20.0));
    }

    #[test]
    fn recent_needs_two_readings_spanning_ten_minutes_inside_half_an_hour() {
        assert_eq!(recent_rate(&[], NOW), None);
        assert_eq!(recent_rate(&[reading(0, 30.0, HOUR)], NOW), None);
        let close = [reading(9 * MIN, 20.0, HOUR), reading(0, 30.0, HOUR)];
        assert_eq!(recent_rate(&close, NOW), None);
        let old = [reading(50 * MIN, 10.0, HOUR), reading(40 * MIN, 20.0, HOUR)];
        assert_eq!(recent_rate(&old, NOW), None);
        let w = window(30.0, Some(2 * HOUR));
        assert_eq!(session_rate(&w, &close, NOW), Some(10.0));
    }

    #[test]
    fn history_keeps_an_hour() {
        let mut history = vec![];
        for (ago, used) in [(70 * MIN, 1.0), (50 * MIN, 2.0), (0, 3.0)] {
            history = push_reading(&history, reading(ago, used, HOUR));
        }
        let kept: Vec<f64> = history.iter().map(|r| r.used_percent).collect();
        assert_eq!(kept, [2.0, 3.0]);
    }

    #[test]
    fn a_reset_clears_the_history() {
        let before = push_reading(&[], reading(20 * MIN, 80.0, 10 * MIN));
        let after_drop = push_reading(&before, reading(0, 5.0, 5 * HOUR));
        assert_eq!(after_drop.len(), 1);
        let moved = push_reading(&before, reading(0, 90.0, 5 * HOUR));
        assert_eq!(moved.len(), 1);
        let jitter = push_reading(&before, reading(0, 90.0, 10 * MIN + 30_000));
        assert_eq!(jitter.len(), 2);
    }
}
