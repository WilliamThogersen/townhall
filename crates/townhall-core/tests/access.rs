use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use serde_json::json;
use tempfile::TempDir;
use townhall_core::model::{normalize_accounts, ForgeAccount, ForgeKind};
use townhall_core::repos::access::{find_account, locate, Found, Located};
use townhall_core::repos::accounts::Picked;
use townhall_core::repos::detect::Detected;
use townhall_core::repos::publish::Programs;

struct Stubs {
    dir: TempDir,
}

impl Stubs {
    fn new() -> Self {
        Self {
            dir: TempDir::new().unwrap(),
        }
    }

    fn write(&self, name: &str, script: &str) -> PathBuf {
        let path = self.dir.path().join(name);
        std::fs::write(&path, script).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    fn log(&self, name: &str) -> String {
        std::fs::read_to_string(self.dir.path().join(format!("{name}.log"))).unwrap_or_default()
    }

    fn gh(&self, sees: &[&str]) -> PathBuf {
        let sees = sees.join(" ");
        let script = format!(
            "#!/bin/sh\necho \"$@\" >> \"{log}\"\ncase \"$1 $2\" in\n\
             \"auth token\") case \" {known} \" in *\" $4 \"*) echo \"tok-$4\"; exit 0;; esac; echo \"no login $4\" >&2; exit 1;;\n\
             \"api --hostname\") echo \"using $GH_TOKEN\" >> \"{log}\"; case \" {sees} \" in *\" ${{GH_TOKEN#tok-}} \"*) echo '{{}}'; exit 0;; esac; echo 'HTTP 404: Not Found' >&2; exit 1;;\n\
             esac\nexit 2\n",
            log = self.dir.path().join("gh.log").display(),
            known = "first second third THIRD",
        );
        self.write("gh", &script)
    }

    fn glab(&self, works: bool) -> PathBuf {
        let api = if works {
            "echo '{}'; exit 0"
        } else {
            "echo '404 Not Found' >&2; exit 1"
        };
        let script = format!(
            "#!/bin/sh\necho \"$@\" >> \"{log}\"\ncase \"$1 $2\" in\n\
             \"config get\") echo \"glpat-$5\"; exit 0;;\n\
             \"api projects/\"*) {api};;\n\
             esac\nexit 2\n",
            log = self.dir.path().join("glab.log").display(),
        );
        self.write("glab", &script)
    }
}

fn programs(gh: &Path, glab: &Path) -> Programs {
    Programs {
        gh: gh.to_string_lossy().into_owned(),
        glab: glab.to_string_lossy().into_owned(),
    }
}

fn registered(list: serde_json::Value) -> Vec<ForgeAccount> {
    normalize_accounts(&list).unwrap()
}

fn github() -> Located {
    locate("git@github.com:acme/shop.git", &[]).unwrap()
}

fn picked(login: &str, host: &str) -> Picked {
    Picked {
        login: login.into(),
        host: host.into(),
    }
}

fn detected(forge: ForgeKind, login: &str, host: &str) -> Detected {
    Detected {
        forge,
        login: login.into(),
        host: host.into(),
        registered: false,
    }
}

#[test]
fn the_first_account_that_can_see_the_repo_wins() {
    let stubs = Stubs::new();
    let programs = programs(&stubs.gh(&["second", "third"]), &stubs.glab(false));
    let list = registered(json!([
        {"forge": "github", "login": "first"},
        {"forge": "github", "login": "second"},
        {"forge": "github", "login": "third"}
    ]));
    let got = find_account(&programs, &github(), &list, &[]);
    assert_eq!(
        got.found,
        Some(Found {
            account: picked("second", "github.com"),
            registered: true
        })
    );
    assert_eq!(
        got.tried,
        vec![
            picked("first", "github.com"),
            picked("second", "github.com")
        ]
    );
    let log = stubs.log("gh");
    assert!(
        log.contains("api --hostname github.com repos/acme/shop"),
        "{log}"
    );
    assert!(!log.contains("using tok-third"), "{log}");
}

#[test]
fn no_account_seeing_the_repo_leaves_it_unpicked_and_lists_who_was_tried() {
    let stubs = Stubs::new();
    let programs = programs(&stubs.gh(&[]), &stubs.glab(false));
    let list = registered(json!([{"forge": "github", "login": "first"}]));
    let seen = [detected(ForgeKind::Github, "second", "github.com")];
    let got = find_account(&programs, &github(), &list, &seen);
    assert_eq!(got.found, None);
    assert_eq!(
        got.tried,
        vec![
            picked("first", "github.com"),
            picked("second", "github.com")
        ]
    );
}

#[test]
fn a_detected_login_is_tried_after_the_registered_ones_and_comes_back_unregistered() {
    let stubs = Stubs::new();
    let programs = programs(&stubs.gh(&["THIRD"]), &stubs.glab(false));
    let list = registered(json!([{"forge": "github", "login": "first"}]));
    let seen = [
        detected(ForgeKind::Github, "first", "github.com"),
        detected(ForgeKind::Github, "THIRD", "github.com"),
        detected(ForgeKind::Github, "other", "github.corp.io"),
        detected(ForgeKind::Gitlab, "gl", "github.com"),
    ];
    let got = find_account(&programs, &github(), &list, &seen);
    assert_eq!(
        got.found,
        Some(Found {
            account: picked("THIRD", "github.com"),
            registered: false
        })
    );
    assert_eq!(got.tried.len(), 2);
}

#[test]
fn accounts_of_another_host_or_forge_are_never_tried() {
    let stubs = Stubs::new();
    let programs = programs(&stubs.gh(&["first"]), &stubs.glab(true));
    let list = registered(json!([
        {"forge": "github", "login": "first", "host": "github.corp.io"},
        {"forge": "gitlab", "login": "gl"}
    ]));
    let got = find_account(&programs, &github(), &list, &[]);
    assert_eq!(got.found, None);
    assert_eq!(got.tried, vec![]);
    assert_eq!(stubs.log("gh"), "");
}

#[test]
fn a_gitlab_project_is_asked_by_its_encoded_path_on_its_host() {
    let stubs = Stubs::new();
    let programs = programs(&stubs.gh(&[]), &stubs.glab(true));
    let located = locate("git@gitlab.example.com:team/sub/app.git", &[]).unwrap();
    let list =
        registered(json!([{"forge": "gitlab", "login": "alice", "host": "gitlab.example.com"}]));
    let got = find_account(&programs, &located, &list, &[]);
    assert_eq!(
        got.found,
        Some(Found {
            account: picked("alice", "gitlab.example.com"),
            registered: true
        })
    );
    let log = stubs.log("glab");
    assert!(
        log.contains("api projects/team%2Fsub%2Fapp --hostname gitlab.example.com"),
        "{log}"
    );
}

#[test]
fn a_detected_gitlab_login_wins_when_nothing_is_registered() {
    let stubs = Stubs::new();
    let programs = programs(&stubs.gh(&[]), &stubs.glab(true));
    let located = locate(
        "https://code.example.com/g/app",
        &["code.example.com".into()],
    )
    .unwrap();
    let seen = [detected(ForgeKind::Gitlab, "alice", "code.example.com")];
    let got = find_account(&programs, &located, &[], &seen);
    assert_eq!(
        got.found,
        Some(Found {
            account: picked("alice", "code.example.com"),
            registered: false
        })
    );
}

#[test]
fn an_account_the_cli_has_no_token_for_counts_as_one_that_cannot_see_the_repo() {
    let stubs = Stubs::new();
    let programs = programs(&stubs.gh(&["ghost"]), &stubs.glab(false));
    let list = registered(json!([
        {"forge": "github", "login": "ghost"},
        {"forge": "github", "login": "first"}
    ]));
    let got = find_account(&programs, &github(), &list, &[]);
    assert_eq!(got.found, None);
    assert_eq!(got.tried.len(), 2);
}

#[test]
fn a_missing_cli_finds_nothing() {
    let programs = Programs {
        gh: "/nonexistent/gh".into(),
        glab: "/nonexistent/glab".into(),
    };
    let list = registered(json!([{"forge": "github", "login": "first"}]));
    let got = find_account(&programs, &github(), &list, &[]);
    assert_eq!(got.found, None);
    assert_eq!(got.tried, vec![picked("first", "github.com")]);
}
