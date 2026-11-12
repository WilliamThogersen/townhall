use townhall_core::clock::parse_rfc3339;

const MINUTE_MS: i64 = 60_000;
const HOUR_MS: i64 = 60 * MINUTE_MS;
const DAY_MS: i64 = 24 * HOUR_MS;
const YEAR_MS: i64 = 365 * DAY_MS;

const STEPS: [(i64, &str); 4] = [
    (YEAR_MS, "y"),
    (DAY_MS, "d"),
    (HOUR_MS, "h"),
    (MINUTE_MS, "m"),
];

pub fn age_label(time: &str, now_ms: i64) -> String {
    let Some(then) = parse_rfc3339(time) else {
        return String::new();
    };
    let elapsed = now_ms - then;
    match STEPS.iter().find(|(ms, _)| elapsed >= *ms) {
        Some((ms, unit)) => format!("{}{unit}", elapsed / ms),
        None => "now".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use townhall_core::clock::{from_unix_millis, parse_rfc3339};

    const MINUTE: i64 = 60_000;
    const NOW: &str = "2026-11-04T12:00:00.000Z";

    fn ago(ms: i64) -> String {
        from_unix_millis(parse_rfc3339(NOW).unwrap() - ms)
    }

    fn now() -> i64 {
        parse_rfc3339(NOW).unwrap()
    }

    #[test]
    fn age_steps() {
        let table: &[(i64, &str)] = &[
            (0, "now"),
            (59_000, "now"),
            (9 * MINUTE, "9m"),
            (59 * MINUTE, "59m"),
            (3 * 60 * MINUTE, "3h"),
            (2 * 24 * 60 * MINUTE, "2d"),
            (400 * 24 * 60 * MINUTE, "1y"),
        ];
        for (ms, label) in table {
            assert_eq!(age_label(&ago(*ms), now()), *label, "{ms} ms ago");
        }
    }

    #[test]
    fn a_time_in_the_future_reads_as_now_and_a_broken_one_as_nothing() {
        assert_eq!(age_label(&ago(-5 * MINUTE), now()), "now");
        assert_eq!(age_label("yesterday", now()), "");
    }
}
