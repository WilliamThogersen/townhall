use std::path::Path;
use std::time::Duration;

use serde_json::json;
use tokio::sync::watch;

use super::args::{self, TurnArgs, DENIED_TOOLS};
use super::process::{Exit, Proc};
use super::prompt;
use super::stream::{parse_line, StreamEvent, TurnResult};
use crate::definitions::{AdapterKind, AgentDef};
use crate::ids;
use crate::model::PermissionMode;
use crate::redact::redact;

const NO_MCP: &str = "{\"mcpServers\":{}}";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Advice {
    pub text: String,
    pub tokens: u64,
    pub model: String,
}

fn tool_name(entry: &str) -> &str {
    entry.split('(').next().unwrap_or(entry)
}

fn read_only(tools: Vec<String>) -> Vec<String> {
    tools
        .into_iter()
        .filter(|t| {
            let name = tool_name(t);
            !name.starts_with("mcp__") && !DENIED_TOOLS.contains(&name)
        })
        .collect()
}

fn permitted(tools: &[String], tool: &str) -> bool {
    tools.iter().any(|t| tool_name(t) == tool)
}

fn mode_of(def: &AgentDef) -> PermissionMode {
    match def.conversation_mode() {
        PermissionMode::AcceptEdits => PermissionMode::Plan,
        other => other,
    }
}

pub async fn consult(
    claude_bin: &str,
    def: &AgentDef,
    cwd: &Path,
    question: &str,
    limit: Duration,
) -> Result<Advice, String> {
    if def.adapter != AdapterKind::Claude {
        return Err(format!(
            "agent type `{}` does not run on claude, so it cannot advise",
            def.name
        ));
    }
    let fields = def.launch_fields();
    let Some(model) = fields.model else {
        return Err(format!(
            "agent type `{}` sets no model: add `model = ...` to it",
            def.name
        ));
    };
    let tools = read_only(fields.allowed_tools);
    if tools.is_empty() {
        return Err(format!(
            "agent type `{}` lists no read-only tools: give it Read, Grep or Glob",
            def.name
        ));
    }
    if !cwd.is_dir() {
        return Err(format!(
            "the folder the adviser reads is gone ({})",
            cwd.display()
        ));
    }
    let argv = args::build(&TurnArgs {
        session_id: ids::new_session_id(),
        resume: false,
        system_prompt: fields.system_prompt.unwrap_or_default(),
        mcp_config: NO_MCP.into(),
        add_dirs: vec![],
        model: model.clone(),
        permission_mode: mode_of(def),
        tools: tools.clone(),
        effort: fields.effort,
        ephemeral: true,
    })
    .map_err(|e| e.to_string())?;
    let run = drive(claude_bin, &argv, cwd, question, &tools);
    match tokio::time::timeout(limit, run).await {
        Ok(done) => done.map(|(text, tokens)| Advice {
            text,
            tokens,
            model,
        }),
        Err(_) => Err(format!(
            "the adviser did not answer within {} seconds",
            limit.as_secs().max(1)
        )),
    }
}

async fn drive(
    claude_bin: &str,
    argv: &[String],
    cwd: &Path,
    question: &str,
    tools: &[String],
) -> Result<(String, u64), String> {
    let mut proc =
        Proc::spawn(claude_bin, argv, cwd).map_err(|e| format!("cannot run claude: {e}"))?;
    proc.write(&prompt::user_line(vec![
        json!({"type": "text", "text": question}),
    ]))
    .await;
    let (_keep, mut stop) = watch::channel(false);
    let mut last_text: Option<String> = None;
    let mut result: Option<TurnResult> = None;
    while let Some(line) = proc.next_line(&mut stop).await {
        for event in parse_line(&line) {
            match event {
                StreamEvent::Text(text) => last_text = Some(text),
                StreamEvent::ToolRequest {
                    request_id,
                    tool,
                    input,
                } => {
                    let reply = match permitted(tools, &tool) {
                        true => prompt::allow_line(&request_id, &input),
                        false => prompt::deny_with(&request_id, &deny_message(tools)),
                    };
                    proc.write(&reply).await;
                }
                StreamEvent::OtherRequest {
                    request_id,
                    subtype,
                } => proc.write(&prompt::error_line(&request_id, &subtype)).await,
                StreamEvent::Result(done) => {
                    result = Some(done);
                    proc.close_stdin();
                }
                StreamEvent::Init | StreamEvent::Usage(_) => {}
            }
        }
    }
    let exit = proc.finish().await;
    outcome(result, &exit, last_text)
}

fn deny_message(tools: &[String]) -> String {
    format!(
        "the adviser may only use {}: it reads and answers, it never edits or runs anything.",
        tools.join(", ")
    )
}

fn outcome(
    result: Option<TurnResult>,
    exit: &Exit,
    last_text: Option<String>,
) -> Result<(String, u64), String> {
    match result {
        Some(done) if done.is_error => {
            let detail = done
                .message
                .as_deref()
                .unwrap_or("the run ended with an error");
            Err(format!("claude failed: {}", redact(detail)))
        }
        Some(done) => {
            let text = done
                .message
                .filter(|m| !m.trim().is_empty())
                .or(last_text)
                .ok_or_else(|| "the adviser ended without an answer".to_string())?;
            Ok((text, done.tokens.unwrap_or(0)))
        }
        None => Err(match (&exit.last_error, exit.code) {
            (Some(line), _) => format!("claude failed: {}", redact(line)),
            (None, Some(code)) => format!("claude exited with {code} before answering"),
            (None, None) => "claude was killed before answering".to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_read_only_non_townhall_tools_stay() {
        let kept = read_only(
            [
                "Read",
                "Edit",
                "Bash(git *)",
                "mcp__townhall__ask_adviser",
                "Grep",
            ]
            .map(String::from)
            .to_vec(),
        );
        assert_eq!(kept, ["Read", "Grep"]);
        assert!(permitted(&kept, "Read") && !permitted(&kept, "Edit"));
    }

    #[test]
    fn the_adviser_never_runs_in_a_mode_that_accepts_edits() {
        let def = |mode: &str| {
            crate::definitions::parse_agent(&format!(
                "name = \"adviser\"\nadapter = \"claude\"\npermission_mode = \"{mode}\"\n"
            ))
            .unwrap()
            .def
        };
        assert_eq!(mode_of(&def("acceptEdits")), PermissionMode::Plan);
        assert_eq!(mode_of(&def("plan")), PermissionMode::Plan);
        assert_eq!(mode_of(&def("bypassPermissions")), PermissionMode::Default);
    }
}
