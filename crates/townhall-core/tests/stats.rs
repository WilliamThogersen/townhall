use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use tempfile::TempDir;
use tokio::process::Command;
use townhall_core::stats::{shipped, ShippedDay};

/// Every commit has this author date, so grouping must use the committer date.
const AUTHORED: &str = "2025-01-01T12:00:00+00:00";
/// 2026-03-01T00:00:00Z, the start of the fixed history below.
const MARCH_2026: u64 = 1_772_323_200;
const DAY: u64 = 86_400;

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// A window from today back to just before the fixed history.
fn since_march_2026() -> u32 {
    u32::try_from((now() - MARCH_2026) / DAY + 2).unwrap()
}

/// One commit: committer date, subject, file and its full contents.
type Change<'a> = (&'a str, &'a str, &'a str, &'a [u8]);

struct Repo {
    dir: TempDir,
}

impl Repo {
    async fn new(date: &str) -> Self {
        let repo = Self {
            dir: TempDir::new().unwrap(),
        };
        repo.git(date, &["init", "-q", "-b", "main"]).await;
        repo.git(date, &["commit", "-q", "--allow-empty", "-m", "init"])
            .await;
        repo
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    async fn git(&self, date: &str, args: &[&str]) -> String {
        let out = Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@t"])
            .args([
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.hooksPath=/dev/null",
            ])
            .args(args)
            .current_dir(self.path())
            .env("GIT_AUTHOR_DATE", AUTHORED)
            .env("GIT_COMMITTER_DATE", date)
            .output()
            .await
            .unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(out.status.success(), "git {args:?}: {stderr}");
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    async fn commit(&self, date: &str, subject: &str, file: &str, body: &[u8]) {
        std::fs::write(self.path().join(file), body).unwrap();
        self.git(date, &["add", file]).await;
        self.git(date, &["commit", "-q", "-m", subject]).await;
    }

    /// Commits `changes` on a new branch `name` off the current one, then returns to main.
    async fn branch(&self, name: &str, changes: &[Change<'_>]) {
        self.checkout(&["-b", name]).await;
        for (date, subject, file, body) in changes {
            self.commit(date, subject, file, body).await;
        }
        self.checkout(&["main"]).await;
    }

    async fn checkout(&self, args: &[&str]) {
        let all = [&["checkout", "-q"], args].concat();
        self.git(AUTHORED, &all).await;
    }

    async fn merge(&self, date: &str, branch: &str, message: Option<&str>) {
        let base = ["merge", "-q", "--no-ff", "--no-edit"];
        let args = match message {
            Some(m) => [&base[..], &["-m", m, branch]].concat(),
            None => [&base[..], &[branch]].concat(),
        };
        self.git(date, &args).await;
    }
}

fn day(date: &str, fixes: u32, features: u32, lines: u64, agents: u32) -> ShippedDay {
    ShippedDay {
        date: date.to_string(),
        fixes,
        features,
        lines,
        agents,
    }
}

const CACHE: &[Change] = &[
    (
        "2026-03-05T10:00:00+00:00",
        "feat!: replace the cache",
        "e.txt",
        b"1\n2\n3\n4\n",
    ),
    (
        "2026-03-05T11:00:00+00:00",
        "fix(cache): evict stale entries",
        "f.txt",
        b"1\n",
    ),
    (
        "2026-03-05T12:00:00+00:00",
        "chore: add logo",
        "logo.bin",
        &[0, 1, 2, 0, 255],
    ),
];
const LOGIN: &[Change] = &[
    (
        "2026-03-02T10:00:00+00:00",
        "feat(auth): add login",
        "a.txt",
        b"1\n2\n3\n",
    ),
    (
        "2026-03-03T10:00:00+00:00",
        "fix: handle expiry",
        "a.txt",
        b"1\n2\n3\n4\n5\n",
    ),
    ("2026-03-04T10:00:00+00:00", "chore: tidy", "b.txt", b"x\n"),
    (
        "2026-03-04T11:00:00+00:00",
        "docs: explain login",
        "c.md",
        b"y\n",
    ),
];
const OTHER: &[Change] = &[(
    "2026-03-11T10:00:00+00:00",
    "feat: not from townhall",
    "g.txt",
    b"z\n",
)];
const DOCS: &[Change] = &[
    (
        "2026-03-11T20:00:00+00:00",
        "docs: readme",
        "h.md",
        b"a\nb\n",
    ),
    (
        "2026-03-11T21:00:00+00:00",
        "refactor: rename",
        "i.txt",
        b"c\n",
    ),
];

/// `townhall/login` (git merge) and `townhall/cache` (pull request, synced with main first)
/// land on 2026-03-10, `feature/other` on 2026-03-11, `townhall/docs` on 2026-03-12
/// (00:30 at +02:00), then a `fix:` straight on main.
async fn history() -> Repo {
    let repo = Repo::new("2026-03-01T10:00:00+00:00").await;
    repo.branch("townhall/cache", CACHE).await;
    repo.branch("townhall/login", LOGIN).await;
    repo.merge("2026-03-10T09:00:00+00:00", "townhall/login", None)
        .await;
    repo.checkout(&["townhall/cache"]).await;
    let sync = "Merge branch 'main' into townhall/cache";
    repo.merge("2026-03-10T12:00:00+00:00", "main", Some(sync))
        .await;
    repo.checkout(&["main"]).await;
    let pr = "Merge pull request #7 from owner/townhall/cache";
    repo.merge("2026-03-10T15:00:00+00:00", "townhall/cache", Some(pr))
        .await;
    repo.branch("feature/other", OTHER).await;
    repo.merge("2026-03-11T12:00:00+00:00", "feature/other", None)
        .await;
    repo.branch("townhall/docs", DOCS).await;
    repo.merge("2026-03-12T00:30:00+02:00", "townhall/docs", None)
        .await;
    let direct = ("2026-03-12T01:00:00+02:00", "fix: straight on main");
    repo.commit(direct.0, direct.1, "j.txt", b"k\n").await;
    repo
}

#[tokio::test]
async fn counts_townhall_merges_per_committer_day() {
    let repo = history().await;
    let days = shipped(repo.path(), "main", since_march_2026())
        .await
        .unwrap();
    assert_eq!(
        days,
        vec![
            day("2026-03-10", 2, 2, 12, 2),
            day("2026-03-12", 0, 0, 3, 1)
        ]
    );
}

#[tokio::test]
async fn only_merges_inside_the_window_count() {
    let repo = Repo::new("2001-01-01T10:00:00+00:00").await;
    let old = [(
        "2001-01-02T10:00:00+00:00",
        "fix: old",
        "a.txt",
        &b"1\n"[..],
    )];
    repo.branch("townhall/old", &old).await;
    repo.merge("2001-01-03T10:00:00+00:00", "townhall/old", None)
        .await;
    let made = format!("@{} +0000", now() - 2 * DAY);
    repo.branch("townhall/new", &[(&made, "feat: new", "b.txt", b"1\n2\n")])
        .await;
    let merged = format!("@{} +0000", now() - DAY);
    repo.merge(&merged, "townhall/new", None).await;

    let days = shipped(repo.path(), "main", 30).await.unwrap();
    assert_eq!(days.len(), 1, "{days:?}");
    assert_eq!((days[0].fixes, days[0].features), (0, 1));
    assert_eq!((days[0].lines, days[0].agents), (2, 1));
    assert!(shipped(repo.path(), "main", 0).await.unwrap().is_empty());
}

#[tokio::test]
async fn a_townhall_branch_merged_into_another_counts_once() {
    let repo = Repo::new("2026-03-01T10:00:00+00:00").await;
    let inner = [("2026-03-02T10:00:00+00:00", "fix: b", "b.txt", &b"1\n"[..])];
    repo.branch("townhall/b", &inner).await;
    repo.checkout(&["-b", "townhall/a"]).await;
    repo.commit("2026-03-03T10:00:00+00:00", "feat: a", "a.txt", b"1\n")
        .await;
    repo.merge("2026-03-06T10:00:00+00:00", "townhall/b", None)
        .await;
    repo.checkout(&["main"]).await;
    repo.merge("2026-03-07T10:00:00+00:00", "townhall/a", None)
        .await;
    let days = shipped(repo.path(), "main", since_march_2026())
        .await
        .unwrap();
    assert_eq!(days, vec![day("2026-03-07", 1, 1, 2, 1)]);
}

#[tokio::test]
async fn a_missing_branch_is_an_error_naming_it() {
    let repo = Repo::new("2026-03-01T10:00:00+00:00").await;
    let err = shipped(repo.path(), "release", 30).await.unwrap_err();
    assert!(err.to_string().contains("`release`"), "{err}");
}

#[tokio::test]
async fn an_option_like_branch_is_rejected() {
    let repo = Repo::new("2026-03-01T10:00:00+00:00").await;
    for bad in ["--all", "", "main two"] {
        let err = shipped(repo.path(), bad, 30).await.unwrap_err();
        assert!(err.to_string().contains(&format!("`{bad}`")), "{err}");
    }
}

#[tokio::test]
async fn a_missing_repo_is_an_error_naming_it() {
    let root = TempDir::new().unwrap();
    let gone = root.path().join("gone");
    let err = shipped(&gone, "main", 30).await.unwrap_err();
    assert!(
        err.to_string().contains(&gone.display().to_string()),
        "{err}"
    );
}

#[tokio::test]
async fn a_folder_that_is_not_a_repo_is_an_error_naming_it() {
    let plain = TempDir::new().unwrap();
    let err = shipped(plain.path(), "main", 30).await.unwrap_err();
    let named = plain.path().display().to_string();
    assert!(err.to_string().contains(&named), "{err}");
}
