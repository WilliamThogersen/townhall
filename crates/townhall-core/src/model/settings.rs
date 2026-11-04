use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};

use crate::{Error, Result};

pub const AGENTS_AT_ONCE_MIN: u32 = 1;
pub const AGENTS_AT_ONCE_MAX: u32 = 50;
pub const AGENTS_AT_ONCE_DEFAULT: u32 = 6;

/// Claude Code's `--permission-mode`, named as in `web/src/settings/settings.ts`.
/// `BypassPermissions` is `skip all`: only `PUT /api/settings/permissionMode` may choose it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionMode {
    Default,
    #[default]
    AcceptEdits,
    Plan,
    Auto,
    BypassPermissions,
}

impl PermissionMode {
    /// The value `--permission-mode` takes; `None` for `skip all`, which is its own flag.
    pub fn cli_value(self) -> Option<&'static str> {
        match self {
            Self::Default => Some("default"),
            Self::AcceptEdits => Some("acceptEdits"),
            Self::Plan => Some("plan"),
            Self::Auto => Some("auto"),
            Self::BypassPermissions => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RepoUnclear {
    #[default]
    Ask,
    Guess,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NotifyWhen {
    #[default]
    NeedsYou,
    Failed,
    Everything,
}

/// One row of the `settings` table: the key is the field name, the value its JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SettingKey {
    PermissionMode,
    WorkWithoutAsking,
    AgentsAtOnce,
    RepoUnclear,
    SessionPanelOpen,
    NotifyWhen,
}

impl SettingKey {
    pub const ALL: [SettingKey; 6] = [
        Self::PermissionMode,
        Self::WorkWithoutAsking,
        Self::AgentsAtOnce,
        Self::RepoUnclear,
        Self::SessionPanelOpen,
        Self::NotifyWhen,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::PermissionMode => "permissionMode",
            Self::WorkWithoutAsking => "workWithoutAsking",
            Self::AgentsAtOnce => "agentsAtOnce",
            Self::RepoUnclear => "repoUnclear",
            Self::SessionPanelOpen => "sessionPanelOpen",
            Self::NotifyWhen => "notifyWhen",
        }
    }

    pub fn parse(key: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|k| k.as_str() == key)
            .ok_or_else(|| Error::InvalidInput(format!("unknown setting `{key}`")))
    }

    /// The value as it will be stored, or why it is not valid for this key.
    pub fn check(self, value: &Value) -> Result<Value> {
        let bad = || Error::InvalidInput(format!("invalid value {value} for `{}`", self.as_str()));
        let valid = match self {
            Self::PermissionMode => parses::<PermissionMode>(value),
            Self::RepoUnclear => parses::<RepoUnclear>(value),
            Self::NotifyWhen => parses::<NotifyWhen>(value),
            Self::WorkWithoutAsking | Self::SessionPanelOpen => value.is_boolean(),
            Self::AgentsAtOnce => value.as_u64().is_some_and(|n| {
                (u64::from(AGENTS_AT_ONCE_MIN)..=u64::from(AGENTS_AT_ONCE_MAX)).contains(&n)
            }),
        };
        if valid {
            Ok(value.clone())
        } else {
            Err(bad())
        }
    }
}

fn parses<T: serde::de::DeserializeOwned>(value: &Value) -> bool {
    serde_json::from_value::<T>(value.clone()).is_ok()
}

/// `GET /api/settings`. Same names, values and defaults as the web UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub permission_mode: PermissionMode,
    pub work_without_asking: bool,
    pub agents_at_once: u32,
    pub repo_unclear: RepoUnclear,
    pub session_panel_open: bool,
    pub notify_when: NotifyWhen,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            permission_mode: PermissionMode::AcceptEdits,
            work_without_asking: false,
            agents_at_once: AGENTS_AT_ONCE_DEFAULT,
            repo_unclear: RepoUnclear::Ask,
            session_panel_open: true,
            notify_when: NotifyWhen::NeedsYou,
        }
    }
}

fn field<T: serde::de::DeserializeOwned>(raw: &Value, key: SettingKey, fallback: T) -> T {
    raw.get(key.as_str())
        .filter(|v| key.check(v).is_ok())
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or(fallback)
}

impl Settings {
    /// Like the web UI's `parseSettings`: a missing or invalid field reads as its default.
    pub fn from_value(raw: &Value) -> Self {
        let d = Self::default();
        Self {
            permission_mode: field(raw, SettingKey::PermissionMode, d.permission_mode),
            work_without_asking: field(raw, SettingKey::WorkWithoutAsking, d.work_without_asking),
            agents_at_once: field(raw, SettingKey::AgentsAtOnce, d.agents_at_once),
            repo_unclear: field(raw, SettingKey::RepoUnclear, d.repo_unclear),
            session_panel_open: field(raw, SettingKey::SessionPanelOpen, d.session_panel_open),
            notify_when: field(raw, SettingKey::NotifyWhen, d.notify_when),
        }
    }

    pub fn from_map(rows: Map<String, Value>) -> Self {
        Self::from_value(&Value::Object(rows))
    }
}

impl<'de> Deserialize<'de> for Settings {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        Value::deserialize(d).map(|raw| Self::from_value(&raw))
    }
}

/// `PUT /api/settings/{key}` body.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PutSetting {
    pub value: Value,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn defaults_match_the_web_ui() {
        let v = serde_json::to_value(Settings::default()).unwrap();
        assert_eq!(
            v,
            json!({
                "permissionMode": "acceptEdits",
                "workWithoutAsking": false,
                "agentsAtOnce": 6,
                "repoUnclear": "ask",
                "sessionPanelOpen": true,
                "notifyWhen": "needsYou"
            })
        );
        let keys: Vec<&str> = SettingKey::ALL.iter().map(|k| k.as_str()).collect();
        let fields: Vec<&String> = v.as_object().unwrap().keys().collect();
        let mut sorted = keys.clone();
        sorted.sort_unstable();
        assert_eq!(fields, sorted);
    }

    #[test]
    fn invalid_fields_fall_back_one_by_one() {
        let raw = json!({"permissionMode": "plan", "agentsAtOnce": 99, "notifyWhen": "x", "repoUnclear": "guess"});
        let s: Settings = serde_json::from_value(raw).unwrap();
        assert_eq!(s.permission_mode, PermissionMode::Plan);
        assert_eq!(s.agents_at_once, AGENTS_AT_ONCE_DEFAULT);
        assert_eq!(s.notify_when, NotifyWhen::NeedsYou);
        assert_eq!(s.repo_unclear, RepoUnclear::Guess);
        assert_eq!(
            serde_json::from_value::<Settings>(json!(7)).unwrap(),
            Settings::default()
        );
    }

    #[test]
    fn keys_check_their_values() {
        let ok = [
            (SettingKey::PermissionMode, json!("bypassPermissions")),
            (SettingKey::AgentsAtOnce, json!(50)),
            (SettingKey::WorkWithoutAsking, json!(true)),
            (SettingKey::NotifyWhen, json!("everything")),
        ];
        for (key, value) in ok {
            assert_eq!(key.check(&value).unwrap(), value);
        }
        let bad = [
            (SettingKey::PermissionMode, json!("yolo")),
            (SettingKey::AgentsAtOnce, json!(0)),
            (SettingKey::AgentsAtOnce, json!(51)),
            (SettingKey::AgentsAtOnce, json!("6")),
            (SettingKey::SessionPanelOpen, json!("yes")),
        ];
        for (key, value) in bad {
            assert!(key.check(&value).is_err(), "{key:?} {value}");
        }
        assert_eq!(
            SettingKey::parse("agentsAtOnce").unwrap(),
            SettingKey::AgentsAtOnce
        );
        assert!(SettingKey::parse("theme").is_err());
    }

    #[test]
    fn permission_values() {
        assert_eq!(PermissionMode::default().cli_value(), Some("acceptEdits"));
        assert_eq!(PermissionMode::Default.cli_value(), Some("default"));
        assert_eq!(PermissionMode::BypassPermissions.cli_value(), None);
    }

    #[test]
    fn put_body_is_strict() {
        let p: PutSetting = serde_json::from_value(json!({"value": 3})).unwrap();
        assert_eq!(p.value, json!(3));
        assert!(serde_json::from_value::<PutSetting>(json!({"value": 3, "key": "x"})).is_err());
        assert!(serde_json::from_value::<PutSetting>(json!({})).is_err());
    }
}
