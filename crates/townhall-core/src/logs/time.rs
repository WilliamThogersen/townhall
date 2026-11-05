//! `since` values: a duration back from now (`90s`, `30m`, `24h`, `7d`, `2w`) or an RFC 3339
//! instant, both as unix milliseconds.

const MS_PER_DAY: i64 = 86_400_000;

/// Milliseconds since the epoch for `since`, relative to `now_ms` for durations.
pub fn parse_since(since: &str, now_ms: i64) -> Result<i64, String> {
    let s = since.trim();
    if let Some(ms) = duration_ms(s) {
        if ms == 0 {
            return Err(format!("`since` must be above zero: {since}"));
        }
        return Ok(now_ms.saturating_sub(ms));
    }
    parse_rfc3339(s).ok_or_else(|| {
        format!("`since` is neither a duration like 24h or 7d nor an RFC 3339 time: {since}")
    })
}

fn duration_ms(s: &str) -> Option<i64> {
    let unit = s.chars().last()?;
    let per: i64 = match unit {
        's' => 1_000,
        'm' => 60_000,
        'h' => 3_600_000,
        'd' => MS_PER_DAY,
        'w' => 7 * MS_PER_DAY,
        _ => return None,
    };
    let digits = &s[..s.len() - 1];
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse::<i64>().ok()?.checked_mul(per)
}

/// `2026-11-05T10:00:00[.fraction](Z|+hh:mm|-hh:mm)` as unix milliseconds.
pub fn parse_rfc3339(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() < 20 || !s.is_ascii() {
        return None;
    }
    let num = |from: usize, to: usize| -> Option<i64> {
        let part = &s[from..to];
        if part.bytes().all(|c| c.is_ascii_digit()) {
            part.parse().ok()
        } else {
            None
        }
    };
    if b[4] != b'-'
        || b[7] != b'-'
        || !(b[10] == b'T' || b[10] == b't')
        || b[13] != b':'
        || b[16] != b':'
    {
        return None;
    }
    let (y, mo, d) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    let (h, mi, sec) = (num(11, 13)?, num(14, 16)?, num(17, 19)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || sec > 60 {
        return None;
    }
    let mut i = 19;
    let mut millis = 0;
    if b[i] == b'.' {
        let start = i + 1;
        i = start;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == start {
            return None;
        }
        let first3: String = s[start..i].chars().chain("000".chars()).take(3).collect();
        millis = first3.parse::<i64>().ok()?;
    }
    let offset_min = match &s[i..] {
        "Z" | "z" => 0,
        tz if tz.len() == 6 && (tz.starts_with('+') || tz.starts_with('-')) && &tz[3..4] == ":" => {
            let oh = tz[1..3].parse::<i64>().ok()?;
            let om = tz[4..6].parse::<i64>().ok()?;
            let total = oh * 60 + om;
            if tz.starts_with('-') {
                -total
            } else {
                total
            }
        }
        _ => return None,
    };
    let days = days_from_civil(y, mo, d);
    Some(days * MS_PER_DAY + ((h * 60 + mi - offset_min) * 60 + sec) * 1000 + millis)
}

/// Howard Hinnant's civil-to-days algorithm (the inverse of `clock`'s).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}
