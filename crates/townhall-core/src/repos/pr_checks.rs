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
            vec!["pr", "checks", &number, "--json", "name,state,bucket"],
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
