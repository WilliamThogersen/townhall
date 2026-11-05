//! The body of `api.anthropic.com/api/oauth/usage` and Claude Code's credentials JSON.
//! Unknown shapes give empty windows and `None`, never a panic.

use serde_json::Value;

use super::{ClaudeUsage, UsageWindow};
use crate::clock::parse_rfc3339;

/// 1e10 sits between any seconds epoch (before 2286) and any milliseconds epoch (after 1970).
const SECONDS_BELOW: f64 = 10_000_000_000.0;

pub fn parse_usage(body: &Value, now: i64) -> ClaudeUsage {
    ClaudeUsage {
        session: window(body.get("five_hour")),
        weekly: window(body.get("seven_day")),
        updated_at: now,
        error: None,
    }
}

pub fn usage_error(status: u16) -> String {
    match status {
        401 | 403 => "Sign in to Claude Code to see usage".to_string(),
        429 => "Usage rate limited, retrying later".to_string(),
        _ => format!("Usage unavailable (HTTP {status})"),
    }
}

/// `claudeAiOauth.accessToken` of Claude Code's credentials JSON.
pub fn token_from_json(raw: &str) -> Option<String> {
    let json: Value = serde_json::from_str(raw).ok()?;
    let token = json.get("claudeAiOauth")?.get("accessToken")?.as_str()?;
    (!token.is_empty()).then(|| token.to_string())
}

fn window(raw: Option<&Value>) -> Option<UsageWindow> {
    let w = raw?.as_object()?;
    let number = |key: &str| w.get(key).and_then(Value::as_f64);
    let used = number("utilization").or_else(|| number("used_percentage"))?;
    Some(UsageWindow {
        used_percent: used.clamp(0.0, 100.0),
        resets_at: w.get("resets_at").and_then(reset_at),
        rate_per_hour: None,
    })
}

fn reset_at(value: &Value) -> Option<i64> {
    let epoch = |n: f64| {
        n.is_finite()
            .then(|| if n > SECONDS_BELOW { n } else { n * 1000.0 }.round() as i64)
    };
    match value {
        Value::Number(n) => epoch(n.as_f64()?),
        Value::String(text) if text.trim().is_empty() => None,
        Value::String(text) => match text.trim().parse::<f64>() {
            Ok(n) => epoch(n),
            Err(_) => parse_rfc3339(text.trim()),
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    const NOW: i64 = 1_793_800_000_000;

    #[test]
    fn reads_both_windows() {
        let body = json!({
            "five_hour": { "utilization": 30.0, "resets_at": "2026-11-05T14:10:00.123456+00:00" },
            "seven_day": { "utilization": 54, "resets_at": "2026-11-11T07:00:00Z" },
        });
        let usage = parse_usage(&body, NOW);
        let session = usage.session.unwrap();
        assert_eq!(session.used_percent, 30.0);
        assert_eq!(session.resets_at, Some(1_793_887_800_123));
        assert_eq!(session.rate_per_hour, None);
        let weekly = usage.weekly.unwrap();
        assert_eq!(weekly.used_percent, 54.0);
        assert_eq!(weekly.resets_at, Some(1_794_380_400_000));
        assert_eq!((usage.updated_at, usage.error), (NOW, None));
    }

    #[test]
    fn reset_times_may_be_seconds_or_milliseconds_as_numbers_or_text() {
        let at = |v: Value| {
            let body = json!({ "five_hour": { "utilization": 1, "resets_at": v } });
            parse_usage(&body, NOW).session.unwrap().resets_at
        };
        assert_eq!(at(json!(1_793_887_800)), Some(1_793_887_800_000));
        assert_eq!(at(json!(1_793_887_800_000_i64)), Some(1_793_887_800_000));
        assert_eq!(at(json!("1793887800")), Some(1_793_887_800_000));
        assert_eq!(at(json!("not a time")), None);
        assert_eq!(at(json!("  ")), None);
        assert_eq!(at(Value::Null), None);
    }

    #[test]
    fn used_percentage_is_the_other_name_and_values_are_clamped() {
        let body = json!({
            "five_hour": { "used_percentage": 140 },
            "seven_day": { "utilization": -5 },
        });
        let usage = parse_usage(&body, NOW);
        assert_eq!(usage.session.unwrap().used_percent, 100.0);
        assert_eq!(usage.weekly.unwrap().used_percent, 0.0);
    }

    #[test]
    fn unknown_shapes_give_empty_windows() {
        for body in [
            json!(null),
            json!([]),
            json!("x"),
            json!({}),
            json!({ "five_hour": null, "seven_day": 3 }),
            json!({ "five_hour": { "utilization": "high" } }),
            json!({ "five_hour": {} }),
        ] {
            let usage = parse_usage(&body, NOW);
            assert_eq!((usage.session, usage.weekly), (None, None), "{body}");
            assert_eq!((usage.updated_at, usage.error), (NOW, None));
        }
    }

    #[test]
    fn http_statuses_have_one_message_each() {
        assert_eq!(usage_error(401), "Sign in to Claude Code to see usage");
        assert_eq!(usage_error(403), "Sign in to Claude Code to see usage");
        assert_eq!(usage_error(429), "Usage rate limited, retrying later");
        assert_eq!(usage_error(500), "Usage unavailable (HTTP 500)");
    }

    #[test]
    fn the_token_is_the_claude_ai_oauth_access_token() {
        let raw = r#"{"claudeAiOauth":{"accessToken":"tok-1","refreshToken":"r"}}"#;
        assert_eq!(token_from_json(raw), Some("tok-1".to_string()));
        for raw in [
            "",
            "not json",
            "{}",
            r#"{"claudeAiOauth":{}}"#,
            r#"{"claudeAiOauth":{"accessToken":""}}"#,
            r#"{"claudeAiOauth":{"accessToken":7}}"#,
        ] {
            assert_eq!(token_from_json(raw), None, "{raw}");
        }
    }
}
