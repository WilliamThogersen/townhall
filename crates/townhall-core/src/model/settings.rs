use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};

use crate::{Error, Result};

pub const AGENTS_AT_ONCE_MIN: u32 = 1;
pub const AGENTS_AT_ONCE_MAX: u32 = 50;
pub const AGENTS_AT_ONCE_DEFAULT: u32 = 6;
/// Most gigabytes `minFreeDiskGb` and `sharedTargetMaxGb` take; `0` turns the limit off.
pub const DISK_GB_MAX: u32 = 100_000;
pub const MIN_FREE_DISK_GB_DEFAULT: u32 = 5;
pub const SHARED_TARGET_MAX_GB_DEFAULT: u32 = 50;

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
    /// The value `claude --permission-mode` takes; `None` for `skip all`, which is its own
    /// flag, `--dangerously-skip-permissions`.
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

/// The model of townhall's own session (`claude --model=<alias>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TownhallModel {
    #[default]
    Sonnet,
    Opus,
    Haiku,
}

impl TownhallModel {
    /// The alias `claude --model` takes.
    pub fn cli_value(self) -> &'static str {
        match self {
            Self::Sonnet => "sonnet",
            Self::Opus => "opus",
            Self::Haiku => "haiku",
        }
    }
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
    TownhallModel,
    SharedCargoTarget,
    MinFreeDiskGb,
    SharedTargetMaxGb,
}

impl SettingKey {
    pub const ALL: [SettingKey; 10] = [
        Self::PermissionMode,
        Self::WorkWithoutAsking,
        Self::AgentsAtOnce,
        Self::RepoUnclear,
        Self::SessionPanelOpen,
        Self::NotifyWhen,
        Self::TownhallModel,
        Self::SharedCargoTarget,
        Self::MinFreeDiskGb,
        Self::SharedTargetMaxGb,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::PermissionMode => "permissionMode",
            Self::WorkWithoutAsking => "workWithoutAsking",
            Self::AgentsAtOnce => "agentsAtOnce",
            Self::RepoUnclear => "repoUnclear",
            Self::SessionPanelOpen => "sessionPanelOpen",
            Self::NotifyWhen => "notifyWhen",
            Self::TownhallModel => "townhallModel",
            Self::SharedCargoTarget => "sharedCargoTarget",
            Self::MinFreeDiskGb => "minFreeDiskGb",
            Self::SharedTargetMaxGb => "sharedTargetMaxGb",
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
            Self::TownhallModel => parses::<TownhallModel>(value),
            Self::WorkWithoutAsking | Self::SessionPanelOpen | Self::SharedCargoTarget => {
                value.is_boolean()
            }
            Self::MinFreeDiskGb | Self::SharedTargetMaxGb => {
                value.as_u64().is_some_and(|n| n <= u64::from(DISK_GB_MAX))
            }
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

/// `GET /api/settings`. Same names, values and defaults as the web UI; `townhallModel`
/// has no row on the settings page yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub permission_mode: PermissionMode,
    pub work_without_asking: bool,
    pub agents_at_once: u32,
    pub repo_unclear: RepoUnclear,
    pub session_panel_open: bool,
    pub notify_when: NotifyWhen,
    pub townhall_model: TownhallModel,
    /// Agents of one repo build into one cargo target folder (`CARGO_TARGET_DIR`).
    pub shared_cargo_target: bool,
    /// A task does not start while the data volume has less free space; `0` is off.
    pub min_free_disk_gb: u32,
    /// The reaper trims a repo's shared target folder above this size; `0` is no cap.
    pub shared_target_max_gb: u32,
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
            townhall_model: TownhallModel::Sonnet,
            shared_cargo_target: true,
            min_free_disk_gb: MIN_FREE_DISK_GB_DEFAULT,
            shared_target_max_gb: SHARED_TARGET_MAX_GB_DEFAULT,
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
            townhall_model: field(raw, SettingKey::TownhallModel, d.townhall_model),
            shared_cargo_target: field(raw, SettingKey::SharedCargoTarget, d.shared_cargo_target),
            min_free_disk_gb: field(raw, SettingKey::MinFreeDiskGb, d.min_free_disk_gb),
            shared_target_max_gb: field(raw, SettingKey::SharedTargetMaxGb, d.shared_target_max_gb),
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
                "notifyWhen": "needsYou",
                "townhallModel": "sonnet",
                "sharedCargoTarget": true,
                "minFreeDiskGb": 5,
                "sharedTargetMaxGb": 50
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
        let raw = json!({"permissionMode": "plan", "agentsAtOnce": 99, "notifyWhen": "x", "repoUnclear": "guess", "townhallModel": "gpt"});
        let s: Settings = serde_json::from_value(raw).unwrap();
        assert_eq!(s.permission_mode, PermissionMode::Plan);
        assert_eq!(s.agents_at_once, AGENTS_AT_ONCE_DEFAULT);
        assert_eq!(s.notify_when, NotifyWhen::NeedsYou);
        assert_eq!(s.repo_unclear, RepoUnclear::Guess);
        assert_eq!(s.townhall_model, TownhallModel::Sonnet);
        let opus: Settings = serde_json::from_value(json!({"townhallModel": "opus"})).unwrap();
        assert_eq!(opus.townhall_model, TownhallModel::Opus);
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
            (SettingKey::TownhallModel, json!("sonnet")),
            (SettingKey::TownhallModel, json!("opus")),
            (SettingKey::TownhallModel, json!("haiku")),
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
            (SettingKey::TownhallModel, json!("fable")),
            (SettingKey::TownhallModel, json!("Sonnet")),
            (SettingKey::TownhallModel, json!("claude-sonnet-4-5")),
            (SettingKey::TownhallModel, json!(null)),
        ];
        for (key, value) in bad {
            assert!(key.check(&value).is_err(), "{key:?} {value}");
        }
        assert_eq!(
            SettingKey::parse("agentsAtOnce").unwrap(),
            SettingKey::AgentsAtOnce
        );
        assert!(SettingKey::parse("theme").is_err());
        assert_eq!(
            SettingKey::parse("townhallModel").unwrap(),
            SettingKey::TownhallModel
        );
    }

    #[test]
    fn disk_settings_check_their_values() {
        let ok = [
            (SettingKey::SharedCargoTarget, json!(false)),
            (SettingKey::MinFreeDiskGb, json!(0)),
            (SettingKey::MinFreeDiskGb, json!(DISK_GB_MAX)),
            (SettingKey::SharedTargetMaxGb, json!(0)),
            (SettingKey::SharedTargetMaxGb, json!(200)),
        ];
        for (key, value) in ok {
            assert_eq!(key.check(&value).unwrap(), value);
        }
        let bad = [
            (SettingKey::SharedCargoTarget, json!("on")),
            (SettingKey::MinFreeDiskGb, json!(-1)),
            (SettingKey::MinFreeDiskGb, json!(DISK_GB_MAX + 1)),
            (SettingKey::MinFreeDiskGb, json!(2.5)),
            (SettingKey::SharedTargetMaxGb, json!("50")),
            (SettingKey::SharedTargetMaxGb, json!(DISK_GB_MAX + 1)),
        ];
        for (key, value) in bad {
            assert!(key.check(&value).is_err(), "{key:?} {value}");
        }
        let s: Settings = serde_json::from_value(json!({"minFreeDiskGb": "x"})).unwrap();
        assert_eq!(s.min_free_disk_gb, MIN_FREE_DISK_GB_DEFAULT);
    }

    #[test]
    fn townhall_models_are_claude_aliases() {
        assert_eq!(TownhallModel::default().cli_value(), "sonnet");
        assert_eq!(TownhallModel::Opus.cli_value(), "opus");
        assert_eq!(TownhallModel::Haiku.cli_value(), "haiku");
    }

    #[test]
    fn permission_values() {
        assert_eq!(PermissionMode::default().cli_value(), Some("acceptEdits"));
        assert_eq!(PermissionMode::Plan.cli_value(), Some("plan"));
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
