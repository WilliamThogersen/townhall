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
    fn now_is_well_formed_and_sorts() {
        let a = now_rfc3339();
        assert_eq!(a.len(), 24, "{a}");
        assert!(a.ends_with('Z') && a.as_bytes()[10] == b'T');
        assert!(now_rfc3339() >= a);
        assert_eq!(today(), a[..10]);
    }
}
