use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use tempfile::TempDir;
use townhall_core::repos::publish::{publish, PrRequest, Programs, Published};

mod common;
use common::{git, init_named};

const BRANCH: &str = "townhall/x";
const AUTH_OK: &str = "exit 0";
const CREATE_GITHUB: &str = "echo https://github.com/acme/shop/pull/7";
const CREATE_GITLAB: &str =
    "echo 'Creating merge request'; echo https://gitlab.example.com/acme/shop/-/merge_requests/12";

struct Env {
    dir: TempDir,
    repo: PathBuf,
    bare: PathBuf,
}

async fn env() -> Env {
    static NO_GIT_CONFIG: std::sync::Once = std::sync::Once::new();
    NO_GIT_CONFIG.call_once(|| {
        std::env::set_var("GIT_CONFIG_GLOBAL", "/dev/null");
        std::env::set_var("GIT_CONFIG_NOSYSTEM", "1");
    });
    let dir = TempDir::new().unwrap();
    let repo = init_named(dir.path(), "shop", "main").await;
    git(&repo, &["checkout", "-q", "-b", BRANCH]).await;
    let commit = ["-c", "user.name=t", "-c", "user.email=t@t"];
    let args: Vec<&str> = commit
        .into_iter()
        .chain(["commit", "-q", "--allow-empty", "-m", "work"])
        .collect();
    git(&repo, &args).await;
    let bare = dir.path().join("bare.git");
    git(
        dir.path(),
        &["init", "-q", "--bare", &bare.to_string_lossy()],
    )
    .await;
    Env { dir, repo, bare }
}

impl Env {
    async fn origin(&self, url: &str) {
        git(&self.repo, &["remote", "add", "origin", url]).await;
        let bare = self.bare.to_string_lossy().into_owned();
        git(&self.repo, &["config", "remote.origin.pushurl", &bare]).await;
    }

    fn stub(&self, name: &str, auth: &str, create: &str) -> PathBuf {
        let path = self.dir.path().join(name);
        let script = format!(
            "#!/bin/sh\necho \"$@\" >> \"{log}\"\nif [ \"$2\" = status ]; then {auth}; fi\n{create}\n",
            log = self.log(name).display()
        );
        std::fs::write(&path, script).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    fn log(&self, name: &str) -> PathBuf {
        self.dir.path().join(format!("{name}.log"))
    }

    fn run(&self, programs: &Programs) -> townhall_core::Result<Published> {
        let req = PrRequest {
            worktree: &self.repo,
            branch: BRANCH,
            base: "main",
            title: "Fix login",
            body: "the body",
            account: "",
        };
        publish(&req, programs)
    }

    fn run_as(&self, programs: &Programs, account: &str) -> townhall_core::Result<Published> {
        let req = PrRequest {
            worktree: &self.repo,
            branch: BRANCH,
            base: "main",
            title: "Fix login",
            body: "the body",
            account,
        };
        publish(&req, programs)
    }

    fn account_stub(&self, active: &str, others: &[&str]) -> PathBuf {
        let state = self.dir.path().join("active");
        std::fs::write(&state, active).unwrap();
        let known = others.join(" ");
        let script = format!(
            "#!/bin/sh\necho \"$@\" >> \"{log}\"\ncase \"$1 $2\" in\n\
             \"auth status\") exit 0;;\n\
             \"auth switch\") case \" {known} \" in *\" $6 \"*) printf %s \"$6\" > \"{state}\"; exit 0;; esac; echo \"no account $6\" >&2; exit 1;;\n\
             \"api --hostname\") echo \"$(cat \"{state}\") 42\";;\n\
             *) {CREATE_GITHUB};;\nesac\n",
            log = self.log("gh").display(),
            state = state.display(),
        );
        let path = self.dir.path().join("gh");
        std::fs::write(&path, script).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    async fn pushed(&self) -> String {
        let bare = self.bare.to_string_lossy().into_owned();
        git(&self.repo, &["ls-remote", &bare, "refs/heads/townhall/x"]).await
    }
}

fn programs(gh: &Path, glab: &Path) -> Programs {
    Programs {
        gh: gh.to_string_lossy().into_owned(),
        glab: glab.to_string_lossy().into_owned(),
    }
}

fn read(path: PathBuf) -> String {
    std::fs::read_to_string(path).unwrap()
}

#[tokio::test]
async fn no_origin_is_a_state_and_runs_nothing() {
    let env = env().await;
    let gh = env.stub("gh", AUTH_OK, CREATE_GITHUB);
    let got = env.run(&programs(&gh, &gh)).unwrap();
    assert_eq!(got, Published::NoRemote);
    assert!(!env.log("gh").exists());
}

#[tokio::test]
async fn github_pushes_then_opens_a_pull_request() {
    let env = env().await;
    env.origin("https://github.com/acme/shop.git").await;
    let gh = env.stub("gh", AUTH_OK, CREATE_GITHUB);
    let got = env
        .run(&programs(&gh, Path::new("/nonexistent/glab")))
        .unwrap();
    let url = "https://github.com/acme/shop/pull/7".to_string();
    assert_eq!(
        got,
        Published::Opened {
            url,
            number: Some(7)
        }
    );
    assert!(!env.pushed().await.is_empty());
    let log = read(env.log("gh"));
    assert!(log.contains("auth status --hostname github.com"), "{log}");
    assert!(
        log.contains("pr create --head townhall/x --base main --title"),
        "{log}"
    );
}

#[tokio::test]
async fn gitlab_uses_glab_and_a_merge_request() {
    let env = env().await;
    env.origin("git@gitlab.example.com:acme/shop.git").await;
    let glab = env.stub("glab", AUTH_OK, CREATE_GITLAB);
    let got = env
        .run(&programs(Path::new("/nonexistent/gh"), &glab))
        .unwrap();
    assert!(
        matches!(
            &got,
            Published::Opened {
                number: Some(12),
                ..
            }
        ),
        "{got:?}"
    );
    let log = read(env.log("glab"));
    assert!(
        log.contains("auth status --hostname gitlab.example.com"),
        "{log}"
    );
    assert!(
        log.contains("mr create --source-branch townhall/x --target-branch main"),
        "{log}"
    );
    assert!(log.contains("--yes"), "{log}");
}

#[tokio::test]
async fn a_host_with_no_rule_fails_before_pushing() {
    let env = env().await;
    env.origin("https://code.example.com/acme/shop.git").await;
    let gh = env.stub("gh", AUTH_OK, CREATE_GITHUB);
    let err = env.run(&programs(&gh, &gh)).unwrap_err().to_string();
    assert!(err.contains("townhall.forge"), "{err}");
    assert!(env.pushed().await.is_empty());
    git(&env.repo, &["config", "townhall.forge", "github"]).await;
    assert!(env.run(&programs(&gh, &gh)).is_ok());
}

#[tokio::test]
async fn a_local_origin_is_refused() {
    let env = env().await;
    let bare = env.bare.to_string_lossy().into_owned();
    git(&env.repo, &["remote", "add", "origin", &bare]).await;
    let gh = env.stub("gh", AUTH_OK, CREATE_GITHUB);
    let err = env.run(&programs(&gh, &gh)).unwrap_err().to_string();
    assert!(err.contains("is a local folder"), "{err}");
    assert!(env.pushed().await.is_empty());
}

#[tokio::test]
async fn a_missing_cli_says_how_to_install_and_log_in() {
    let env = env().await;
    env.origin("https://github.com/acme/shop.git").await;
    let missing = Path::new("/nonexistent/gh");
    let err = env
        .run(&programs(missing, missing))
        .unwrap_err()
        .to_string();
    assert!(err.contains("is not installed"), "{err}");
    assert!(err.contains("gh auth login --hostname github.com"), "{err}");
    assert!(env.pushed().await.is_empty());
}

#[tokio::test]
async fn a_logged_out_cli_is_told_apart() {
    let env = env().await;
    env.origin("https://github.com/acme/shop.git").await;
    let gh = env.stub("gh", "echo 'not logged in' >&2; exit 1", CREATE_GITHUB);
    let err = env.run(&programs(&gh, &gh)).unwrap_err().to_string();
    assert!(err.contains("not logged in to github.com"), "{err}");
    assert!(env.pushed().await.is_empty());
}

#[tokio::test]
async fn an_existing_pull_request_is_returned() {
    let env = env().await;
    env.origin("https://github.com/acme/shop.git").await;
    let create = "echo 'a pull request for branch townhall/x already exists:' >&2; echo https://github.com/acme/shop/pull/3 >&2; exit 1";
    let gh = env.stub("gh", AUTH_OK, create);
    let got = env.run(&programs(&gh, &gh)).unwrap();
    let url = "https://github.com/acme/shop/pull/3".to_string();
    assert_eq!(
        got,
        Published::Opened {
            url,
            number: Some(3)
        }
    );
}

#[tokio::test]
async fn a_create_without_a_link_or_that_fails_is_an_error() {
    let env = env().await;
    env.origin("https://github.com/acme/shop.git").await;
    let silent = env.stub("silent", AUTH_OK, "echo done");
    let err = env
        .run(&programs(&silent, &silent))
        .unwrap_err()
        .to_string();
    assert!(err.contains("printed no link"), "{err}");
    let failing = env.stub("failing", AUTH_OK, "echo boom >&2; exit 1");
    let err = env
        .run(&programs(&failing, &failing))
        .unwrap_err()
        .to_string();
    assert!(err.contains("`gh pr create` failed: boom"), "{err}");
}

#[tokio::test]
async fn a_push_that_fails_is_an_error() {
    let env = env().await;
    env.origin("https://github.com/acme/shop.git").await;
    git(
        &env.repo,
        &["config", "remote.origin.pushurl", "/nonexistent/folder.git"],
    )
    .await;
    let gh = env.stub("gh", AUTH_OK, CREATE_GITHUB);
    let err = env.run(&programs(&gh, &gh)).unwrap_err().to_string();
    assert!(
        err.starts_with("invalid input: could not push townhall/x to origin"),
        "{err}"
    );
    let log = read(env.log("gh"));
    assert!(!log.contains("pr create"), "{log}");
}

const ME: &str = "WilliamThogersen";

#[tokio::test]
async fn the_active_account_is_kept_and_becomes_the_git_identity() {
    let env = env().await;
    env.origin("https://github.com/acme/shop.git").await;
    let gh = env.account_stub(ME, &[ME]);
    let got = env.run_as(&programs(&gh, &gh), ME).unwrap();
    assert!(matches!(got, Published::Opened { .. }), "{got:?}");
    let log = read(env.log("gh"));
    assert!(!log.contains("auth switch"), "{log}");
    let name = git(&env.repo, &["config", "--get", "user.name"]).await;
    let email = git(&env.repo, &["config", "--get", "user.email"]).await;
    assert_eq!(name.trim(), ME);
    assert_eq!(email.trim(), "42+WilliamThogersen@users.noreply.github.com");
}

#[tokio::test]
async fn another_active_account_is_switched_before_anything_is_pushed() {
    let env = env().await;
    env.origin("https://github.com/acme/shop.git").await;
    let gh = env.account_stub("someone-else", &[ME, "someone-else"]);
    env.run_as(&programs(&gh, &gh), ME).unwrap();
    let log = read(env.log("gh"));
    assert!(
        log.contains(&format!("auth switch --hostname github.com --user {ME}")),
        "{log}"
    );
    assert!(!env.pushed().await.is_empty());
}

#[tokio::test]
async fn an_account_gh_has_no_login_for_fails_clearly_and_pushes_nothing() {
    let env = env().await;
    env.origin("https://github.com/acme/shop.git").await;
    let gh = env.account_stub("someone-else", &["someone-else"]);
    let err = env.run_as(&programs(&gh, &gh), ME).unwrap_err().to_string();
    assert!(
        err.contains(ME) && err.contains("gh auth login --hostname github.com"),
        "{err}"
    );
    assert!(env.pushed().await.is_empty());
    let log = read(env.log("gh"));
    assert!(!log.contains("pr create"), "{log}");
}

#[tokio::test]
async fn no_expected_account_means_no_account_calls() {
    let env = env().await;
    env.origin("https://github.com/acme/shop.git").await;
    let gh = env.account_stub("someone-else", &[]);
    env.run_as(&programs(&gh, &gh), "").unwrap();
    let log = read(env.log("gh"));
    assert!(
        !log.contains("auth switch") && !log.contains("api "),
        "{log}"
    );
}
