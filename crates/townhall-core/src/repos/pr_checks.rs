use std::path::Path;
use std::process::Command;

use serde::Deserialize;

use super::accounts::Accounts;
use super::publish::{
    github_session, gitlab_session, origin_of, with_session, Forge, Programs, Session,
};
use crate::redact::redact;
use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckState {
    pub name: String,
    pub state: String,
    pub bucket: String,
    pub link: String,
}

impl CheckState {
    pub fn pending(&self) -> bool {
        self.bucket == "pending"
    }

    pub fn failed(&self) -> bool {
        self.bucket == "fail" || self.bucket == "cancel"
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrFacts {
    pub head: String,
    pub conflicting: bool,
    pub unknown: bool,
    pub behind: u64,
    pub state: String,
    pub merge_state: String,
    pub merge_commit: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenPr {
    pub number: u64,
    pub url: String,
    pub branch: String,
    pub head: String,
    pub base: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Updated {
    Started,
    Conflict(String),
}

pub fn job_id(link: &str) -> Option<u64> {
    let (_, rest) = link.split_once("/job/")?;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

const LOG_LINES: usize = 40;
const LOG_CHARS: usize = 3000;

fn log_tail(log: &str) -> String {
    let lines: Vec<&str> = log.lines().collect();
    let kept: Vec<String> = lines[lines.len().saturating_sub(LOG_LINES)..]
        .iter()
        .map(|line| redact(line).into_owned())
        .collect();
    let joined = kept.join("\n");
    let chars: Vec<char> = joined.trim_end().chars().collect();
    chars[chars.len().saturating_sub(LOG_CHARS)..]
        .iter()
        .collect()
}

#[derive(Debug, Clone, Copy)]
pub struct ChecksRequest<'a> {
    pub worktree: &'a Path,
    pub number: u64,
    pub branch: &'a str,
    pub accounts: &'a Accounts,
}

#[derive(Debug, Deserialize)]
struct GhCheck {
    name: String,
    #[serde(default)]
    state: String,
    #[serde(default)]
    bucket: String,
    #[serde(default)]
    link: String,
}

#[derive(Debug, Deserialize)]
struct GlabPipeline {
    status: String,
}

fn invalid(message: String) -> Error {
    Error::InvalidInput(redact(&message).into_owned())
}

fn gitlab_bucket(status: &str) -> &'static str {
    match status {
        "created" | "waiting_for_resource" | "preparing" | "pending" | "running" | "scheduled" => {
            "pending"
        }
        "success" => "pass",
        "failed" => "fail",
        "canceled" | "canceling" => "cancel",
        _ => "skipping",
    }
}

fn parse(forge: Forge, stdout: &str) -> Option<Vec<CheckState>> {
    match forge {
        Forge::GitHub => {
            let checks: Vec<GhCheck> = serde_json::from_str(stdout).ok()?;
            Some(
                checks
                    .into_iter()
                    .map(|c| CheckState {
                        name: c.name,
                        state: c.state,
                        bucket: c.bucket,
                        link: c.link,
                    })
                    .collect(),
            )
        }
        Forge::GitLab => {
            let pipeline: GlabPipeline = serde_json::from_str(stdout).ok()?;
            Some(vec![CheckState {
                name: "pipeline".to_string(),
                bucket: gitlab_bucket(&pipeline.status).to_string(),
                state: pipeline.status,
                link: String::new(),
            }])
        }
    }
}

pub fn pr_checks(req: &ChecksRequest, programs: &Programs) -> Result<Option<Vec<CheckState>>> {
    let Some(origin) = origin_of(req.worktree)? else {
        return Ok(None);
    };
    let program = match origin.forge {
        Forge::GitHub => &programs.gh,
        Forge::GitLab => &programs.glab,
    };
    let session = match req.accounts.pick(origin.forge.kind(), &origin.host)? {
        Some(picked) => match origin.forge {
            Forge::GitHub => github_session(program, &picked, req.worktree)?,
            Forge::GitLab => gitlab_session(program, &picked, req.worktree)?,
        },
        None => Session::new(),
    };
    let number = req.number.to_string();
    let (shown, args, extra): (&str, Vec<&str>, (&str, &str)) = match origin.forge {
        Forge::GitHub => (
            "gh pr checks",
            vec!["pr", "checks", &number, "--json", "name,state,bucket,link"],
            ("GH_PROMPT_DISABLED", "1"),
        ),
        Forge::GitLab => (
            "glab ci get",
            vec!["ci", "get", "--branch", req.branch, "--output", "json"],
            ("GITLAB_HOST", origin.host.as_str()),
        ),
    };
    let mut envs = with_session(&session);
    envs.push(extra);
    let output = Command::new(program)
        .args(&args)
        .current_dir(req.worktree)
        .envs(envs)
        .output()
        .map_err(|e| Error::io(format!("run `{shown}`"), e))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if let Some(checks) = parse(origin.forge, stdout.trim()) {
        return Ok(Some(checks));
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    if origin.forge == Forge::GitHub && stderr.to_ascii_lowercase().contains("no checks reported") {
        return Ok(Some(Vec::new()));
    }
    Err(invalid(format!(
        "`{shown}` for #{} gave no checks: {}",
        req.number,
        stderr.trim()
    )))
}

struct Gh {
    program: String,
    envs: Vec<(String, String)>,
}

fn github_of(req: &ChecksRequest, programs: &Programs) -> Result<Option<Gh>> {
    let Some(origin) = origin_of(req.worktree)? else {
        return Ok(None);
    };
    if origin.forge != Forge::GitHub {
        return Ok(None);
    }
    let session = match req.accounts.pick(origin.forge.kind(), &origin.host)? {
        Some(picked) => github_session(&programs.gh, &picked, req.worktree)?,
        None => Session::new(),
    };
    let mut envs: Vec<(String, String)> = with_session(&session)
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    envs.push(("GH_PROMPT_DISABLED".to_string(), "1".to_string()));
    Ok(Some(Gh {
        program: programs.gh.clone(),
        envs,
    }))
}

fn gh_run(gh: &Gh, worktree: &Path, args: &[&str]) -> Result<String> {
    let shown = format!(
        "gh {}",
        args.iter().take(2).copied().collect::<Vec<_>>().join(" ")
    );
    let output = Command::new(&gh.program)
        .args(args)
        .current_dir(worktree)
        .envs(gh.envs.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .output()
        .map_err(|e| Error::io(format!("run `{shown}`"), e))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(invalid(format!("`{shown}` failed: {}", stderr.trim())));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhView {
    head_ref_oid: String,
    mergeable: String,
    #[serde(default = "open_state")]
    state: String,
    #[serde(default)]
    merge_state_status: String,
    #[serde(default)]
    merge_commit: Option<GhCommit>,
}

#[derive(Debug, Deserialize)]
struct GhCommit {
    oid: String,
}

fn open_state() -> String {
    "OPEN".to_string()
}

fn parse_view(json: &str, number: u64) -> Result<GhView> {
    serde_json::from_str(json)
        .map_err(|e| invalid(format!("`gh pr view` for #{number} gave no facts: {e}")))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhListed {
    number: u64,
    url: String,
    head_ref_name: String,
    head_ref_oid: String,
    base_ref_name: String,
}

fn parse_open_prs(json: &str) -> Result<Vec<OpenPr>> {
    let listed: Vec<GhListed> = serde_json::from_str(json)
        .map_err(|e| invalid(format!("`gh pr list` gave no pull requests: {e}")))?;
    Ok(listed
        .into_iter()
        .map(|p| OpenPr {
            number: p.number,
            url: p.url,
            branch: p.head_ref_name,
            head: p.head_ref_oid,
            base: p.base_ref_name,
        })
        .collect())
}

pub fn pr_facts(req: &ChecksRequest, base: &str, programs: &Programs) -> Result<Option<PrFacts>> {
    let Some(gh) = github_of(req, programs)? else {
        return Ok(None);
    };
    let number = req.number.to_string();
    let shown = gh_run(
        &gh,
        req.worktree,
        &[
            "pr",
            "view",
            &number,
            "--json",
            "headRefOid,mergeable,state,mergeStateStatus,mergeCommit",
        ],
    )?;
    let view = parse_view(&shown, req.number)?;
    let range = format!(
        "repos/{{owner}}/{{repo}}/compare/{base}...{}",
        view.head_ref_oid
    );
    let behind = gh_run(&gh, req.worktree, &["api", &range, "--jq", ".behind_by"])?;
    let behind = behind
        .parse()
        .map_err(|_| invalid(format!("`gh api` gave `{behind}` as commits behind {base}")))?;
    Ok(Some(PrFacts {
        conflicting: view.mergeable == "CONFLICTING",
        unknown: view.mergeable == "UNKNOWN",
        head: view.head_ref_oid,
        behind,
        state: view.state,
        merge_state: view.merge_state_status,
        merge_commit: view.merge_commit.map(|c| c.oid),
    }))
}

pub fn open_prs(req: &ChecksRequest, programs: &Programs) -> Result<Option<Vec<OpenPr>>> {
    let Some(gh) = github_of(req, programs)? else {
        return Ok(None);
    };
    let listed = gh_run(
        &gh,
        req.worktree,
        &[
            "pr",
            "list",
            "--state",
            "open",
            "--limit",
            "200",
            "--json",
            "number,url,headRefName,headRefOid,baseRefName",
        ],
    )?;
    parse_open_prs(&listed).map(Some)
}

pub fn rerun_job(req: &ChecksRequest, job: u64, programs: &Programs) -> Result<Option<()>> {
    let Some(gh) = github_of(req, programs)? else {
        return Ok(None);
    };
    gh_run(
        &gh,
        req.worktree,
        &["run", "rerun", "--job", &job.to_string()],
    )?;
    Ok(Some(()))
}

pub fn failed_log(req: &ChecksRequest, job: u64, programs: &Programs) -> Result<Option<String>> {
    let Some(gh) = github_of(req, programs)? else {
        return Ok(None);
    };
    let log = gh_run(
        &gh,
        req.worktree,
        &["run", "view", "--job", &job.to_string(), "--log-failed"],
    )?;
    Ok(Some(log_tail(&log)))
}

pub fn update_branch(
    req: &ChecksRequest,
    head: &str,
    programs: &Programs,
) -> Result<Option<Updated>> {
    let Some(gh) = github_of(req, programs)? else {
        return Ok(None);
    };
    let path = format!(
        "repos/{{owner}}/{{repo}}/pulls/{}/update-branch",
        req.number
    );
    let expected = format!("expected_head_sha={head}");
    match gh_run(
        &gh,
        req.worktree,
        &["api", "-X", "PUT", &path, "-f", &expected],
    ) {
        Ok(_) => Ok(Some(Updated::Started)),
        Err(Error::InvalidInput(m)) if m.to_ascii_lowercase().contains("conflict") => {
            Ok(Some(Updated::Conflict(m)))
        }
        Err(e) => Err(e),
    }
}

pub fn merge_pr(req: &ChecksRequest, head: &str, programs: &Programs) -> Result<Option<String>> {
    let Some(gh) = github_of(req, programs)? else {
        return Ok(None);
    };
    let number = req.number.to_string();
    gh_run(
        &gh,
        req.worktree,
        &[
            "pr",
            "merge",
            &number,
            "--merge",
            "--match-head-commit",
            head,
        ],
    )?;
    let sha = gh_run(
        &gh,
        req.worktree,
        &[
            "pr",
            "view",
            &number,
            "--json",
            "mergeCommit",
            "--jq",
            ".mergeCommit.oid",
        ],
    )?;
    Ok(Some(sha))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_checks_keep_their_name_state_and_bucket() {
        let json = r#"[{"name":"build","state":"QUEUED","bucket":"pending"},{"name":"lint","state":"FAILURE","bucket":"fail"}]"#;
        let got = parse(Forge::GitHub, json).unwrap();
        assert_eq!(got.len(), 2);
        assert!(got[0].pending() && !got[0].failed());
        assert!(got[1].failed() && !got[1].pending());
        assert_eq!(got[1].state, "FAILURE");
        assert_eq!(parse(Forge::GitHub, "[]"), Some(vec![]));
        assert_eq!(parse(Forge::GitHub, "not json"), None);
    }

    #[test]
    fn a_job_id_comes_from_the_check_link() {
        assert_eq!(
            job_id("https://github.com/o/r/actions/runs/123/job/456"),
            Some(456)
        );
        assert_eq!(
            job_id("https://github.com/o/r/actions/runs/123/job/456?pr=7"),
            Some(456)
        );
        assert_eq!(job_id("https://github.com/o/r/actions/runs/123"), None);
        assert_eq!(job_id("https://example.com/status"), None);
        assert_eq!(job_id(""), None);
    }

    #[test]
    fn checks_keep_their_link() {
        let json = r#"[{"name":"a","state":"FAILURE","bucket":"fail","link":"https://x/job/9"},{"name":"b","bucket":"pass"}]"#;
        let got = parse(Forge::GitHub, json).unwrap();
        assert_eq!(got[0].link, "https://x/job/9");
        assert_eq!(got[1].link, "");
    }

    #[test]
    fn a_failed_log_keeps_the_last_40_lines_and_3000_characters() {
        let long: String = (1..=100).map(|i| format!("line {i}\n")).collect();
        let tail = log_tail(&long);
        assert_eq!(tail.lines().count(), 40);
        assert!(tail.starts_with("line 61"));
        assert!(tail.ends_with("line 100"));
        let wide: String = (0..40)
            .map(|i| format!("{i:02} {}end\n", "abc def ".repeat(12)))
            .collect();
        let tail = log_tail(&wide);
        assert_eq!(tail.chars().count(), 3000);
        assert!(tail.ends_with("end"));
        assert_eq!(log_tail("short\n"), "short");
    }

    #[test]
    fn a_failed_log_is_redacted() {
        let tail = log_tail("token ghp_abcdefghijklmnopqrstuvwxyz0123456789 leaked\n");
        assert!(
            !tail.contains("ghp_abcdefghijklmnopqrstuvwxyz0123456789"),
            "{tail}"
        );
    }

    #[test]
    fn open_pull_requests_are_read_from_pr_list_json() {
        let json = r#"[{"number":7,"url":"https://github.com/o/r/pull/7","headRefName":"townhall/a","headRefOid":"abc","baseRefName":"main"}]"#;
        let got = parse_open_prs(json).unwrap();
        assert_eq!(
            got,
            vec![OpenPr {
                number: 7,
                url: "https://github.com/o/r/pull/7".into(),
                branch: "townhall/a".into(),
                head: "abc".into(),
                base: "main".into(),
            }]
        );
        assert_eq!(parse_open_prs("[]").unwrap(), vec![]);
        assert!(parse_open_prs("nope").is_err());
    }

    #[test]
    fn a_view_without_state_or_merge_state_reads_as_open_and_empty() {
        let view = parse_view(r#"{"headRefOid":"abc","mergeable":"MERGEABLE"}"#, 7).unwrap();
        assert_eq!(view.state, "OPEN");
        assert_eq!(view.merge_state_status, "");
        assert!(view.merge_commit.is_none());
        let full = parse_view(
            r#"{"headRefOid":"abc","mergeable":"MERGEABLE","state":"MERGED","mergeStateStatus":"BLOCKED","mergeCommit":{"oid":"def"}}"#,
            7,
        )
        .unwrap();
        assert_eq!(
            (full.state.as_str(), full.merge_state_status.as_str()),
            ("MERGED", "BLOCKED")
        );
        assert_eq!(full.merge_commit.map(|c| c.oid), Some("def".to_string()));
        let open = parse_view(
            r#"{"headRefOid":"abc","mergeable":"MERGEABLE","mergeCommit":null}"#,
            7,
        )
        .unwrap();
        assert!(open.merge_commit.is_none());
    }

    #[test]
    fn a_gitlab_pipeline_is_one_check() {
        for (status, pending) in [("running", true), ("pending", true), ("success", false)] {
            let got = parse(Forge::GitLab, &format!(r#"{{"status":"{status}"}}"#)).unwrap();
            assert_eq!(got.len(), 1);
            assert_eq!(
                (got[0].name.as_str(), got[0].pending()),
                ("pipeline", pending)
            );
        }
        let failed = parse(Forge::GitLab, r#"{"status":"failed"}"#).unwrap();
        assert!(failed[0].failed());
    }
}
