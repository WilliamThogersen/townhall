//! Transcript timestamps (`2026-11-04T10:42:05.123Z`, UTC) as unix milliseconds.

const MS_PER_DAY: i64 = 86_400_000;

/// Howard Hinnant's days-from-civil algorithm.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn number(text: &str, range: std::ops::Range<usize>) -> Option<i64> {
    let part = text.get(range)?;
    if part.bytes().all(|b| b.is_ascii_digit()) {
        part.parse().ok()
    } else {
        None
    }
}

/// Milliseconds of a fraction like `.123` or `.5`, and what follows it.
fn fraction(rest: &str) -> Option<(i64, &str)> {
    let Some(digits) = rest.strip_prefix('.') else {
        return Some((0, rest));
    };
    let len = digits.bytes().take_while(u8::is_ascii_digit).count();
    let millis: String = digits[..len].chars().chain("000".chars()).take(3).collect();
    Some((millis.parse().ok()?, &digits[len..]))
}

/// Unix milliseconds of a UTC RFC 3339 time; `None` for anything else.
pub(crate) fn millis(text: &str) -> Option<i64> {
    let seps = [(4, b'-'), (7, b'-'), (10, b'T'), (13, b':'), (16, b':')];
    if !seps
        .iter()
        .all(|&(i, c)| text.as_bytes().get(i) == Some(&c))
    {
        return None;
    }
    let [y, mo, d, h, mi, s] = [0..4, 5..7, 8..10, 11..13, 14..16, 17..19].map(|r| number(text, r));
    let (y, mo, d, h, mi, s) = (y?, mo?, d?, h?, mi?, s?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || s > 60 {
        return None;
    }
    let (ms, zone) = fraction(text.get(19..)?)?;
    if zone != "Z" && zone != "+00:00" {
        return None;
    }
    let secs = ((h * 60) + mi) * 60 + s;
    Some(days_from_civil(y, mo, d) * MS_PER_DAY + secs * 1000 + ms)
}

#[cfg(test)]
mod tests {
    use super::millis;
    use crate::clock::from_unix_millis;

    #[test]
    fn reads_what_the_clock_writes() {
        for ms in [0, 951_782_400_000, 1_793_788_925_000, 1_793_836_800_123] {
            assert_eq!(millis(&from_unix_millis(ms)), Some(ms));
        }
        assert_eq!(millis("2026-11-04T10:42:05Z"), Some(1_793_788_925_000));
        assert_eq!(
            millis("2026-11-04T10:42:05.5+00:00"),
            Some(1_793_788_925_500)
        );
    }

    #[test]
    fn refuses_other_shapes() {
        for bad in [
            "",
            "2026-11-04",
            "2026-13-04T10:42:05Z",
            "2026-11-04T10:42:05+02:00",
            "2026-11-04 10:42:05Z",
            "2026-11-04T1x:42:05Z",
        ] {
            assert_eq!(millis(bad), None, "{bad}");
        }
    }
}
