use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};

use super::accounts::{normalize as normalize_accounts, ForgeAccount};
use crate::{Error, Result};

pub const AGENTS_AT_ONCE_MIN: u32 = 1;
pub const AGENTS_AT_ONCE_MAX: u32 = 50;
pub const AGENTS_AT_ONCE_DEFAULT: u32 = 6;
pub const DISK_GB_MAX: u32 = 100_000;
pub const MIN_FREE_DISK_GB_DEFAULT: u32 = 5;

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
    MinFreeDiskGb,
    LearnAboutMe,
    GithubAccount,
    ForgeAccounts,
    ReflectAfterTasks,
    ClonesDir,
}

impl SettingKey {
    pub const ALL: [SettingKey; 12] = [
        Self::PermissionMode,
        Self::WorkWithoutAsking,
        Self::AgentsAtOnce,
        Self::RepoUnclear,
        Self::SessionPanelOpen,
        Self::NotifyWhen,
        Self::MinFreeDiskGb,
        Self::LearnAboutMe,
        Self::GithubAccount,
        Self::ForgeAccounts,
        Self::ReflectAfterTasks,
        Self::ClonesDir,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::PermissionMode => "permissionMode",
            Self::WorkWithoutAsking => "workWithoutAsking",
            Self::AgentsAtOnce => "agentsAtOnce",
            Self::RepoUnclear => "repoUnclear",
            Self::SessionPanelOpen => "sessionPanelOpen",
            Self::NotifyWhen => "notifyWhen",
            Self::MinFreeDiskGb => "minFreeDiskGb",
            Self::LearnAboutMe => "learnAboutMe",
            Self::GithubAccount => "githubAccount",
            Self::ForgeAccounts => "forgeAccounts",
            Self::ReflectAfterTasks => "reflectAfterTasks",
            Self::ClonesDir => "clonesDir",
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
        if self == Self::ForgeAccounts {
            let accounts = normalize_accounts(value)?;
            return serde_json::to_value(accounts)
                .map_err(|e| Error::InvalidInput(format!("invalid forge accounts: {e}")));
        }
        let valid = match self {
            Self::PermissionMode => parses::<PermissionMode>(value),
            Self::RepoUnclear => parses::<RepoUnclear>(value),
            Self::NotifyWhen => parses::<NotifyWhen>(value),
            Self::WorkWithoutAsking
            | Self::SessionPanelOpen
            | Self::LearnAboutMe
            | Self::ReflectAfterTasks => value.is_boolean(),
            Self::MinFreeDiskGb => value.as_u64().is_some_and(|n| n <= u64::from(DISK_GB_MAX)),
            Self::GithubAccount => value.as_str().is_some_and(is_github_login),
            Self::ClonesDir => value.as_str().is_some_and(is_clones_dir),
            Self::ForgeAccounts => unreachable!("handled above"),
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

/// A GitHub login, or empty for "no account is enforced".
fn is_github_login(login: &str) -> bool {
    login.len() <= 39
        && !login.starts_with('-')
        && login.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// Empty (townhall's own `repos` folder), `~`, `~/...` or an absolute path.
pub fn is_clones_dir(dir: &str) -> bool {
    dir.is_empty() || dir == "~" || dir.starts_with("~/") || dir.starts_with('/')
}

fn parses<T: serde::de::DeserializeOwned>(value: &Value) -> bool {
    serde_json::from_value::<T>(value.clone()).is_ok()
}

/// `GET /api/settings`. Same names, values and defaults as the web UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub permission_mode: PermissionMode,
    pub work_without_asking: bool,
    pub agents_at_once: u32,
    pub repo_unclear: RepoUnclear,
    pub session_panel_open: bool,
    pub notify_when: NotifyWhen,
    /// A task does not start while the data volume has less free space; `0` is off.
    pub min_free_disk_gb: u32,
    pub learn_about_me: bool,
    /// The old single account: still read as the fallback for a GitHub repo with no
    /// registered account of its own.
    pub github_account: String,
    /// The registered forge accounts; each repo picks one per forge.
    pub forge_accounts: Vec<ForgeAccount>,
    /// After a task ends, a `reflector` run records lessons in the repo's memory files.
    pub reflect_after_tasks: bool,
    /// The folder a repo added by link is cloned into; empty means `<home>/repos`.
    pub clones_dir: String,
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
            min_free_disk_gb: MIN_FREE_DISK_GB_DEFAULT,
            learn_about_me: false,
            github_account: String::new(),
            forge_accounts: Vec::new(),
            reflect_after_tasks: false,
            clones_dir: String::new(),
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
            min_free_disk_gb: field(raw, SettingKey::MinFreeDiskGb, d.min_free_disk_gb),
            learn_about_me: field(raw, SettingKey::LearnAboutMe, d.learn_about_me),
            github_account: field(raw, SettingKey::GithubAccount, d.github_account),
            forge_accounts: field(raw, SettingKey::ForgeAccounts, d.forge_accounts),
            reflect_after_tasks: field(raw, SettingKey::ReflectAfterTasks, d.reflect_after_tasks),
            clones_dir: field(raw, SettingKey::ClonesDir, d.clones_dir),
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

/// `PUT /api/profile` body.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PutProfile {
    pub text: String,
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
                "minFreeDiskGb": 5,
                "learnAboutMe": false,
                "githubAccount": "",
                "forgeAccounts": [],
                "reflectAfterTasks": false,
                "clonesDir": ""
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
        // a leftover stored `townhallModel` is ignored, not an error
        let s: Settings = serde_json::from_value(raw).unwrap();
        assert_eq!(s.permission_mode, PermissionMode::Plan);
        assert_eq!(s.agents_at_once, AGENTS_AT_ONCE_DEFAULT);
        assert_eq!(s.notify_when, NotifyWhen::NeedsYou);
        assert_eq!(s.repo_unclear, RepoUnclear::Guess);
        let old: Settings = serde_json::from_value(json!({"townhallModel": "opus"})).unwrap();
        assert_eq!(old, Settings::default());
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
        assert!(SettingKey::parse("townhallModel").is_err());
    }

    #[test]
    fn disk_settings_check_their_values() {
        let ok = [
            (SettingKey::MinFreeDiskGb, json!(0)),
            (SettingKey::MinFreeDiskGb, json!(DISK_GB_MAX)),
        ];
        for (key, value) in ok {
            assert_eq!(key.check(&value).unwrap(), value);
        }
        let bad = [
            (SettingKey::MinFreeDiskGb, json!(-1)),
            (SettingKey::MinFreeDiskGb, json!(DISK_GB_MAX + 1)),
            (SettingKey::MinFreeDiskGb, json!(2.5)),
        ];
        for (key, value) in bad {
            assert!(key.check(&value).is_err(), "{key:?} {value}");
        }
        let s: Settings = serde_json::from_value(json!({"minFreeDiskGb": "x"})).unwrap();
        assert_eq!(s.min_free_disk_gb, MIN_FREE_DISK_GB_DEFAULT);
    }

    #[test]
    fn keys_of_removed_settings_are_ignored() {
        let raw = json!({
            "agentsAtOnce": 3, "sharedCargoTarget": true, "sharedTargetMaxGb": 80,
            "minFreeDiskGb": 7
        });
        let s: Settings = serde_json::from_value(raw).unwrap();
        assert_eq!((s.agents_at_once, s.min_free_disk_gb), (3, 7));
        assert!(SettingKey::parse("sharedCargoTarget").is_err());
        assert!(SettingKey::parse("sharedTargetMaxGb").is_err());
    }

    #[test]
    fn learn_about_me_is_off_unless_stored_as_a_boolean() {
        let old: Settings = serde_json::from_value(json!({"permissionMode": "plan"})).unwrap();
        assert!(!old.learn_about_me, "a settings file from before the field");
        let on: Settings = serde_json::from_value(json!({"learnAboutMe": true})).unwrap();
        assert!(on.learn_about_me);
        let bad: Settings = serde_json::from_value(json!({"learnAboutMe": "yes"})).unwrap();
        assert!(!bad.learn_about_me);
        assert_eq!(
            SettingKey::parse("learnAboutMe").unwrap(),
            SettingKey::LearnAboutMe
        );
        assert!(SettingKey::LearnAboutMe.check(&json!(true)).is_ok());
        assert!(SettingKey::LearnAboutMe.check(&json!(1)).is_err());
    }

    #[test]
    fn reflect_after_tasks_is_off_unless_stored_as_a_boolean() {
        assert!(!Settings::default().reflect_after_tasks);
        let old: Settings = serde_json::from_value(json!({"permissionMode": "plan"})).unwrap();
        assert!(!old.reflect_after_tasks);
        let on: Settings = serde_json::from_value(json!({"reflectAfterTasks": true})).unwrap();
        assert!(on.reflect_after_tasks);
        let bad: Settings = serde_json::from_value(json!({"reflectAfterTasks": "yes"})).unwrap();
        assert!(!bad.reflect_after_tasks);
        assert_eq!(
            SettingKey::parse("reflectAfterTasks").unwrap(),
            SettingKey::ReflectAfterTasks
        );
        assert!(SettingKey::ReflectAfterTasks.check(&json!(true)).is_ok());
        assert!(SettingKey::ReflectAfterTasks.check(&json!(1)).is_err());
    }

    #[test]
    fn the_github_account_is_a_login_or_empty() {
        for good in ["WilliamThogersen", "a", "octo-cat-1", ""] {
            let value = json!(good);
            assert_eq!(SettingKey::GithubAccount.check(&value).unwrap(), value);
        }
        for bad in [json!("-lead"), json!("two words"), json!("a/b"), json!(7)] {
            assert!(SettingKey::GithubAccount.check(&bad).is_err(), "{bad}");
        }
        let s: Settings =
            serde_json::from_value(json!({"githubAccount": "WilliamThogersen"})).unwrap();
        assert_eq!(s.github_account, "WilliamThogersen");
        let bad: Settings = serde_json::from_value(json!({"githubAccount": "no way"})).unwrap();
        assert_eq!(bad.github_account, "");
    }

    #[test]
    fn forge_accounts_are_checked_and_stored_with_their_defaults_filled_in() {
        let given = json!([{"forge": "gitlab", "login": "me", "host": "GitLab.Corp.io"}]);
        let stored = SettingKey::ForgeAccounts.check(&given).unwrap();
        assert_eq!(
            stored,
            json!([{"forge": "gitlab", "login": "me", "host": "gitlab.corp.io", "label": "me"}])
        );
        let s: Settings = serde_json::from_value(json!({"forgeAccounts": stored})).unwrap();
        assert_eq!(s.forge_accounts.len(), 1);
        assert_eq!(s.forge_accounts[0].host, "gitlab.corp.io");
        assert!(SettingKey::ForgeAccounts.check(&json!("me")).is_err());
        let bad: Settings = serde_json::from_value(json!({"forgeAccounts": "me"})).unwrap();
        assert!(bad.forge_accounts.is_empty());
        assert_eq!(
            SettingKey::parse("forgeAccounts").unwrap(),
            SettingKey::ForgeAccounts
        );
    }

    #[test]
    fn the_clones_dir_is_empty_or_an_absolute_or_home_folder() {
        for good in ["", "~/Developer", "~", "/srv/code"] {
            let value = json!(good);
            assert_eq!(
                SettingKey::ClonesDir.check(&value).unwrap(),
                value,
                "{good}"
            );
        }
        for bad in [
            json!("relative/dir"),
            json!("  "),
            json!("~other"),
            json!(7),
            json!(null),
        ] {
            assert!(SettingKey::ClonesDir.check(&bad).is_err(), "{bad}");
        }
        let s: Settings = serde_json::from_value(json!({"clonesDir": "~/Developer"})).unwrap();
        assert_eq!(s.clones_dir, "~/Developer");
        let bad: Settings = serde_json::from_value(json!({"clonesDir": "x"})).unwrap();
        assert_eq!(bad.clones_dir, "");
        assert_eq!(
            SettingKey::parse("clonesDir").unwrap(),
            SettingKey::ClonesDir
        );
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
