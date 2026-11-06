//! UTC timestamps as RFC 3339 strings (`2026-11-04T10:42:05.000Z`), without a date library.
//! They sort as text in time order, which the stores rely on.

use std::time::{SystemTime, UNIX_EPOCH};

const MS_PER_DAY: i64 = 86_400_000;

pub fn now_rfc3339() -> String {
    from_unix_millis(unix_millis(SystemTime::now()))
}

/// Today's UTC date as `YYYY-MM-DD`.
pub fn today() -> String {
    date_of(unix_millis(SystemTime::now()))
}

pub fn unix_millis(t: SystemTime) -> i64 {
    match t.duration_since(UNIX_EPOCH) {
        Ok(d) => i64::try_from(d.as_millis()).unwrap_or(i64::MAX),
        Err(e) => -i64::try_from(e.duration().as_millis()).unwrap_or(i64::MAX),
    }
}

pub fn from_unix_millis(ms: i64) -> String {
    let day_ms = ms.rem_euclid(MS_PER_DAY);
    let (secs, millis) = (day_ms / 1000, day_ms % 1000);
    format!(
        "{}T{:02}:{:02}:{:02}.{millis:03}Z",
        date_of(ms),
        secs / 3600,
        secs % 3600 / 60,
        secs % 60
    )
}

/// Unix milliseconds of an RFC 3339 time (`2026-11-05T14:10:00.123456+00:00`, `Z` or a
/// numeric offset). Fractions are cut to milliseconds. `None` for anything else.
pub fn parse_rfc3339(text: &str) -> Option<i64> {
    let b = text.as_bytes();
    let digits = |from: usize, len: usize| -> Option<i64> {
        let part = b.get(from..from + len)?;
        part.iter()
            .all(u8::is_ascii_digit)
            .then(|| part.iter().fold(0, |n, d| n * 10 + i64::from(d - b'0')))
    };
    let at = |i: usize, want: &[u8]| b.get(i).is_some_and(|c| want.contains(c));
    let (year, month, day) = (digits(0, 4)?, digits(5, 2)?, digits(8, 2)?);
    let (hour, minute, second) = (digits(11, 2)?, digits(14, 2)?, digits(17, 2)?);
    let shaped = at(4, b"-") && at(7, b"-") && at(10, b"Tt ") && at(13, b":") && at(16, b":");
    let ranges = (1..=12).contains(&month) && (1..=31).contains(&day);
    if !shaped || !ranges || hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    let mut end = 19;
    let mut millis = 0;
    if at(end, b".") {
        let fraction = b[end + 1..].iter().take_while(|c| c.is_ascii_digit());
        let kept: Vec<i64> = fraction.map(|c| i64::from(c - b'0')).collect();
        if kept.is_empty() {
            return None;
        }
        millis = (0..3).fold(0, |n, i| n * 10 + kept.get(i).copied().unwrap_or(0));
        end += 1 + kept.len();
    }
    let offset_ms = match text.get(end..)? {
        "Z" | "z" => 0,
        zone if zone.len() == 6 && at(end, b"+-") && at(end + 3, b":") => {
            let minutes = digits(end + 1, 2)? * 60 + digits(end + 4, 2)?;
            let sign = if at(end, b"-") { -1 } else { 1 };
            sign * minutes * 60_000
        }
        _ => return None,
    };
    let days = days_from_civil(year, month, day);
    let secs = days * 86_400 + hour * 3600 + minute * 60 + second;
    Some(secs * 1000 + millis - offset_ms)
}

/// Days since 1970-01-01 of a `YYYY-MM-DD` date (four, two and two digits, month 1-12, day
/// 1-31). `None` for anything else.
pub fn day_number(text: &str) -> Option<i64> {
    let mut parts = text.split('-');
    let (y, m, d) = (parts.next()?, parts.next()?, parts.next()?);
    let shaped = parts.next().is_none()
        && (y.len(), m.len(), d.len()) == (4, 2, 2)
        && [y, m, d]
            .iter()
            .all(|p| p.bytes().all(|b| b.is_ascii_digit()));
    if !shaped {
        return None;
    }
    let (y, m, d) = (y.parse().ok()?, m.parse().ok()?, d.parse().ok()?);
    ((1..=12).contains(&m) && (1..=31).contains(&d)).then(|| days_from_civil(y, m, d))
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = y - i64::from(m <= 2);
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let doy = (153 * ((m + 9) % 12) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn date_of(ms: i64) -> String {
    let (y, m, d) = civil_from_days(ms.div_euclid(MS_PER_DAY));
    format!("{y:04}-{m:02}-{d:02}")
}

/// Howard Hinnant's days-to-civil algorithm.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_known_instants() {
        assert_eq!(from_unix_millis(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(
            from_unix_millis(951_782_400_000),
            "2000-02-29T00:00:00.000Z"
        );
        assert_eq!(
            from_unix_millis(1_793_836_800_123),
            "2026-11-05T00:00:00.123Z"
        );
        assert_eq!(
            from_unix_millis(1_793_788_925_000),
            "2026-11-04T10:42:05.000Z"
        );
        assert_eq!(from_unix_millis(-1000), "1969-12-31T23:59:59.000Z");
    }

    #[test]
    fn parses_what_it_formats() {
        for ms in [0, 951_782_400_000, 1_793_836_800_123, -1000] {
            assert_eq!(parse_rfc3339(&from_unix_millis(ms)), Some(ms));
        }
    }

    #[test]
    fn parses_offsets_and_long_fractions() {
        let utc = Some(1_793_887_800_123);
        assert_eq!(parse_rfc3339("2026-11-05T14:10:00.123456+00:00"), utc);
        assert_eq!(
            parse_rfc3339("2026-11-05T14:10:00.12Z"),
            Some(1_793_887_800_120)
        );
        assert_eq!(parse_rfc3339("2026-11-05T15:10:00.123+01:00"), utc);
        assert_eq!(parse_rfc3339("2026-11-05T09:40:00.123-04:30"), utc);
        assert_eq!(
            parse_rfc3339("2026-11-05T14:10:00Z"),
            Some(1_793_887_800_000)
        );
    }

    #[test]
    fn refuses_what_is_not_a_time() {
        for text in [
            "",
            "tomorrow",
            "1793887800",
            "2026-11-05",
            "2026-11-05T14:10:00",
            "2026-11-05T14:10:00.Z",
            "2026-13-05T14:10:00Z",
            "2026-11-05T25:10:00Z",
            "2026-11-05T14:10:00+0100",
            "2026-11-05T14:10:00Zjunk",
        ] {
            assert_eq!(parse_rfc3339(text), None, "{text}");
        }
    }

    #[test]
    fn day_number_matches_known_dates() {
        assert_eq!(day_number("1970-01-01"), Some(0));
        assert_eq!(day_number("2024-02-01"), Some(19754));
        assert_eq!(
            day_number("2024-03-01").unwrap() - day_number("2024-02-01").unwrap(),
            29
        );
    }

    #[test]
    fn day_number_refuses_what_is_not_a_date() {
        for text in [
            "",
            "2024-13-01",
            "2024-00-01",
            "2024-01-32",
            "2024-01-00",
            "24-1-1",
            "2024-1-01",
            "2024-01-1",
            "2024-01-01-01",
            "2024-01",
            "+024-01-01",
            "2024-01-0a",
            "2024-01-01T00:00:00Z",
        ] {
            assert_eq!(day_number(text), None, "{text}");
        }
    }

    #[test]
    fn now_is_well_formed_and_sorts() {
        let a = now_rfc3339();
        assert_eq!(a.len(), 24, "{a}");
        assert!(a.ends_with('Z') && a.as_bytes()[10] == b'T');
        assert!(now_rfc3339() >= a);
        assert_eq!(today(), a[..10]);
    }
}
