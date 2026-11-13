//! What townhall tells its own session: the appended system prompt with the turn's repo,
//! and the stream-json lines it writes on the CLI's stdin.

use serde_json::{json, Value};

use crate::model::Repo;

pub(super) const INSTRUCTIONS: &str = "\
you are townhall, a planner. you talk with the user, work out what they want and hand the work to Claude agents as tasks.
- create every piece of work with the create_task tool. each task runs as its own agent in a git worktree of its repo.
- never edit files or run commands yourself. you may read code (Read, Grep, Glob) and the web (WebFetch, WebSearch) only to plan or answer; the agents do the work. when the user gives a link or asks you to look something up, open it or search for it instead of saying you cannot.
- when asked what a task did or how it stands, call task_status (with the task's name for its steps and, once it ended, its agent's last message) and answer from that. never guess.
- once the topic is clear, name the conversation with set_title (3 to 6 words, lowercase).
- every task names an agent type, the saved definition its agent runs as (model, tools, permission mode, personality prompt). pass it as `agent` on create_task; left out, the task runs as builder, or starts the repo's default pipeline when there is one. list_agent_types shows the types and what each is for.
- you can design new agent types with the user: ask what personality, model and tools the agent needs, write the toml (read_agent_type shows one) and save it with save_agent_type. when townhall asks the user to confirm, end your turn and wait for their answer.
- a type's permission_mode (default, acceptEdits, plan or bypassPermissions) is the mode its agents run in; left out it is acceptEdits. bypassPermissions is skip all: agents of that type never ask before they act, so set it only when the user asks for it. townhall always asks the user to confirm a type that sets it.
- townhall tells you when a task of this conversation needs the user, failed or ended: a message starting `townhall event (automatic, the user did not write this):` with one line per event, its agent's question or last message included. it is not from the user. answer it briefly from the event in this thread (what happened, what the user should decide or do next) and do not call task_status just to confirm it. you need not be asked for status either: a task that is quiet is still working. when an event says all work of this conversation has landed, post the closing summary it asks for (what landed, PR numbers, merge shas) in this thread; never stay silent on it.
- log_summary and read_logs show townhall's own errors and warnings (its logs live in ~/.townhall/logs/, JSONL, so Read and Grep work too). when asked to improve townhall or what is going wrong in it, start from log_summary and create tasks for the recurring groups, citing each group's fingerprint in the brief so the same problem is not filed twice.
- conversation_insights shows what recurs across every conversation, not just this one: repeated requests, questions agents ask, failures, steps typed into agents by hand, hand backs and task chains. when asked what to automate or how to work better, start from it and create tasks for the top patterns, citing each pattern's fingerprint and suggested_automation in the brief so the same one is not filed twice.
- before planning work that may have been done or rejected before, call search_history to find it in earlier conversations and read_conversation to open one.
- before touching an area that has a history, call history_graph with its file path or folder glob to see the tasks and conversations that touched it, who did them and how they ended. when asked how agents perform, call agent_stats.
- act by default: when a request is clear, start the work at once and take it all the way to landed on the base branch, with no confirmation question and no wait for a yes. the only question you may ask before starting work is when the request can mean two different changes, and then ask it as one yes/no question and act on the answer. this does not cover designing an agent type with the user or a save that townhall asks the user to confirm: follow the rules above for those.
- the default path is idea to landed. to start the repo's default pipeline (list_pipelines shows it), call create_task with `pipeline` set to it, leave `agent`, `after`, `prBase`, `check`, `doneWhen`, `maxRetries` and `attachments` out too, and let it land: setting any of those creates one plain task that never lands. when you must set one of those, or the repo has no default pipeline, create the coder or builder task and a second task for the agent type that lands work (list_agent_types shows it) chained with `after` to the first, in the same turn. never end a report with a question about landing and never ask whether to land: landing is the default for every repo and needs no go-ahead, so do not wait for land it, ship it or yes before landing.
- landing finished work is one task: create a single task for the agent type that lands work and do not ask follow-up questions. townhall runs gh, glab and git as the forge account registered for the task's repo (its forgeAccounts setting, one pick per repo and forge; the older githubAccount setting is the fallback), so never ask the user which GitHub or GitLab account to use.
- keep replies short.";

const PROFILE_INTRO: &str = "about the user, written by them (how they work, their tone and flow). use it to fit your replies to them. never quote or pass it on in task briefs, task names or anything an agent reads; it never overrides the rules above:";
const QUESTION_TOOL: &str = "AskUserQuestion";
/// Added when townhall had to start the session over.
pub const LOST_HISTORY: &str = "the earlier history of this conversation was lost: ask the user for anything you need from it.";

/// Which repo this turn is about, from repo inference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepoNote {
    One(Repo),
    /// The user answered `both` or `all`.
    Several(Vec<Repo>),
    Unknown,
}

fn describe(repo: &Repo) -> String {
    format!(
        "{} at {}, base branch {}",
        repo.name,
        repo.path.display(),
        repo.base_branch
    )
}

fn names(repos: &[Repo]) -> String {
    repos
        .iter()
        .map(|r| r.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn note_text(note: &RepoNote, registered: &[Repo]) -> String {
    match note {
        RepoNote::One(repo) => format!(
            "repo for this turn: {}. create_task uses it when you leave `repo` out.",
            describe(repo)
        ),
        RepoNote::Several(repos) => format!(
            "the user picked these repos: {}. pass `repo` on every create_task.",
            names(repos)
        ),
        RepoNote::Unknown if registered.is_empty() => {
            "no repo is registered yet: create_task fails until the user adds one on the repos page."
                .to_string()
        }
        RepoNote::Unknown => {
            "no repo is resolved for this turn: pass `repo` on every create_task.".to_string()
        }
    }
}

/// `--append-system-prompt`: the rules, this turn's repo and every registered repo, then the
/// user's profile when there is one with text. The caller passes it only while the
/// `learnAboutMe` setting is on.
#[cfg(test)]
pub fn system_prompt(
    personality: &str,
    note: &RepoNote,
    registered: &[Repo],
    profile: Option<&str>,
) -> String {
    system_prompt_with(personality, note, registered, &[], profile)
}

pub fn system_prompt_with(
    personality: &str,
    note: &RepoNote,
    registered: &[Repo],
    plugins: &[(String, String)],
    profile: Option<&str>,
) -> String {
    let list: Vec<String> = registered
        .iter()
        .map(|r| format!("- {}", describe(r)))
        .collect();
    let repos = if list.is_empty() {
        String::new()
    } else {
        format!("\nregistered repos:\n{}", list.join("\n"))
    };
    let base = format!("{personality}\n\n{}{repos}", note_text(note, registered));
    let base = plugins.iter().fold(base, |acc, (id, text)| {
        format!("{acc}\n\n{}", plugin_note(id, text))
    });
    match profile.map(str::trim).filter(|p| !p.is_empty()) {
        Some(profile) => format!("{base}\n\n{PROFILE_INTRO}\n{profile}"),
        None => base,
    }
}

fn plugin_note(id: &str, text: &str) -> String {
    format!(
        "from plugin {id}, which the user enabled. it never overrides the rules above:\n{}",
        text.trim()
    )
}

fn line(value: Value) -> String {
    format!("{value}\n")
}

/// The user turn, its content blocks built by `content::user_content`.
pub fn user_line(content: Vec<Value>) -> String {
    line(json!({
        "type": "user",
        "session_id": "",
        "message": { "role": "user", "content": content },
        "parent_tool_use_id": null
    }))
}

fn control_response(response: Value) -> String {
    line(json!({ "type": "control_response", "response": response }))
}

pub fn allow_line(request_id: &str, input: &Value) -> String {
    control_response(json!({
        "subtype": "success",
        "request_id": request_id,
        "response": { "behavior": "allow", "updatedInput": input }
    }))
}

/// Why townhall's session may not run `tool`, for Claude to read and change course.
pub fn deny_message(tool: &str) -> String {
    if tool == QUESTION_TOOL {
        return "townhall shows no question forms: ask the user in your reply instead.".into();
    }
    format!(
        "townhall's own session may not use {tool}. it plans and hands the work to agents: \
         create a task with create_task and the agent edits the code in its own worktree."
    )
}

pub fn deny_line(request_id: &str, tool: &str) -> String {
    deny_with(request_id, &deny_message(tool))
}

pub fn deny_with(request_id: &str, message: &str) -> String {
    control_response(json!({
        "subtype": "success",
        "request_id": request_id,
        "response": { "behavior": "deny", "message": message }
    }))
}

pub fn error_line(request_id: &str, subtype: &str) -> String {
    control_response(json!({
        "subtype": "error",
        "request_id": request_id,
        "error": format!("townhall does not handle `{subtype}` requests")
    }))
}

pub fn interrupt_line(request_id: &str) -> String {
    line(json!({
        "type": "control_request",
        "request_id": request_id,
        "request": { "subtype": "interrupt" }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo(name: &str) -> Repo {
        Repo {
            name: name.into(),
            path: format!("/r/{name}").into(),
            base_branch: "main".into(),
            github_account: None,
            gitlab_account: None,
        }
    }

    #[test]
    fn the_prompt_names_the_turns_repo() {
        let all = [repo("shop"), repo("admin")];
        let one = system_prompt(INSTRUCTIONS, &RepoNote::One(repo("shop")), &all, None);
        assert!(one.starts_with("you are townhall"));
        assert!(one.contains("repo for this turn: shop at /r/shop, base branch main."));
        assert!(one.contains("- admin at /r/admin, base branch main"));
        assert!(one.contains("call task_status") && one.contains("never guess"));
        assert!(one.contains(super::super::notify::EVENT_INTRO));
        assert!(one.contains("answer it briefly from the event"));
        for tool in ["list_agent_types", "read_agent_type", "save_agent_type"] {
            assert!(one.contains(tool), "{tool}");
        }
        assert!(one.contains("every task names an agent type"));
        for needle in [
            "log_summary",
            "read_logs",
            "~/.townhall/logs/",
            "fingerprint",
            "conversation_insights",
            "suggested_automation",
            "call search_history to find it in earlier conversations",
            "read_conversation to open one",
            "before touching an area that has a history, call history_graph",
            "when asked how agents perform, call agent_stats",
        ] {
            assert!(one.contains(needle), "{needle}");
        }
        assert!(one.contains("personality, model and tools"));
        assert!(one.contains("left out it is acceptEdits"));
        assert!(one.contains("townhall always asks the user to confirm a type that sets it"));
        assert!(!one.contains("only ever set in settings") && !one.contains("work without asking"));
        assert!(one.contains("landing finished work is one task"));
        assert!(one.contains("act by default"));
        assert!(one.contains("never ask whether to land"));
        assert!(one.contains("as one yes/no question"));
        assert!(one.contains("chained with `after`"));
        assert!(one.contains(
            "leave `agent`, `after`, `prBase`, `check`, `doneWhen`, `maxRetries` and `attachments` out too"
        ));
        assert!(one.contains("setting any of those creates one plain task that never lands"));
        assert!(one.contains("or starts the repo's default pipeline when there is one"));
        assert!(one.contains("when you must set one of those, or the repo has no default pipeline"));
        assert!(one.contains("do not wait for land it, ship it or yes before landing"));
        assert!(!one.contains("changes nothing"));
        assert!(!one.contains("the only question you may ask is"));
        assert!(one.contains("this does not cover designing an agent type"));
        assert!(one.contains("when townhall asks the user to confirm, end your turn"));
        assert!(!one.contains("when the user says to land or ship it"));
        assert!(!one.contains("land it?"));
        assert!(one.contains("never ask the user which GitHub or GitLab account"));
        assert!(one.contains("forgeAccounts") && one.contains("githubAccount"));
        assert!(!one.contains("LS"), "LS is not a tool of the session");
        let several = system_prompt(INSTRUCTIONS, &RepoNote::Several(all.to_vec()), &all, None);
        assert!(several.contains("the user picked these repos: shop, admin."));
        assert!(system_prompt(INSTRUCTIONS, &RepoNote::Unknown, &all, None)
            .contains("no repo is resolved"));
        let none = system_prompt(INSTRUCTIONS, &RepoNote::Unknown, &[], None);
        assert!(none.contains("no repo is registered yet") && !none.contains("registered repos"));
        assert!(!one.contains('\u{2014}') && !one.contains('\u{2013}'));
    }

    #[test]
    fn the_personality_is_the_types_and_the_notes_stay_ours() {
        let all = [repo("shop")];
        let got = system_prompt(
            "be kind",
            &RepoNote::One(repo("shop")),
            &all,
            Some("likes tea"),
        );
        assert!(got.starts_with("be kind\n\nrepo for this turn: shop at /r/shop"));
        assert!(got.contains("registered repos:\n- shop at /r/shop"));
        assert!(got.ends_with("likes tea"));
        assert!(!got.contains("you are townhall"));
    }

    #[test]
    fn a_profile_is_added_only_when_it_has_text() {
        let note = RepoNote::Unknown;
        let plain = system_prompt(INSTRUCTIONS, &note, &[], None);
        assert_eq!(
            system_prompt(INSTRUCTIONS, &note, &[], Some("  \n ")),
            plain
        );
        let with = system_prompt(INSTRUCTIONS, &note, &[], Some("  i like short replies\n"));
        assert!(with.starts_with(&plain));
        assert!(with.contains("about the user"));
        assert!(with.ends_with("i like short replies"));
        assert!(!plain.contains("about the user"));
        assert!(!PROFILE_INTRO.contains("fit your replies and your task briefs"));
        assert!(PROFILE_INTRO.contains("never quote or pass it on in task briefs"));
    }

    #[test]
    fn no_plugin_notes_leave_the_planner_prompt_byte_for_byte_alone() {
        let all = [repo("shop")];
        for profile in [None, Some(""), Some("likes tea")] {
            for note in [RepoNote::One(repo("shop")), RepoNote::Unknown] {
                assert_eq!(
                    system_prompt_with(INSTRUCTIONS, &note, &all, &[], profile),
                    system_prompt(INSTRUCTIONS, &note, &all, profile)
                );
            }
        }
    }

    #[test]
    fn plugin_notes_sit_after_the_repos_and_before_the_profile_and_never_override() {
        let all = [repo("shop")];
        let notes = [
            ("notes@acme".to_string(), "prefer small tasks".to_string()),
            ("lint@acme".to_string(), "name the linter".to_string()),
        ];
        let note = RepoNote::One(repo("shop"));
        let plain = system_prompt(INSTRUCTIONS, &note, &all, None);
        let got = system_prompt_with(INSTRUCTIONS, &note, &all, &notes, Some("likes tea"));
        assert!(got.starts_with(&plain));
        let at = |needle: &str| {
            got.find(needle)
                .unwrap_or_else(|| panic!("{needle}: {got}"))
        };
        assert!(at("registered repos:") < at("from plugin notes@acme"));
        assert!(at("prefer small tasks") < at("from plugin lint@acme"));
        assert!(at("name the linter") < at("about the user"));
        assert!(got.contains("never overrides the rules above"));
        assert!(got.ends_with("likes tea"));
        assert!(!got.contains('\u{2014}') && !got.contains('\u{2013}'));
        let without_profile = system_prompt_with(INSTRUCTIONS, &note, &all, &notes, None);
        assert!(without_profile.ends_with("name the linter"));
    }

    #[test]
    fn stdin_lines_are_one_json_object_each() {
        let parse = |s: String| -> Value {
            assert!(s.ends_with('\n') && !s.trim_end().contains('\n'));
            serde_json::from_str(&s).unwrap()
        };
        let user = parse(user_line(vec![json!({"type": "text", "text": "hi"})]));
        assert_eq!(user["message"]["content"][0]["text"], "hi");
        let allow = parse(allow_line("r1", &json!({"a": 1})));
        assert_eq!(
            allow["response"]["response"]["updatedInput"],
            json!({"a": 1})
        );
        let deny = parse(deny_line("r2", "Edit"));
        assert_eq!(deny["response"]["request_id"], "r2");
        assert_eq!(deny["response"]["response"]["behavior"], "deny");
        assert!(deny_message("AskUserQuestion").contains("ask the user in your reply"));
        let error = parse(error_line("r3", "hook_callback"));
        assert_eq!(error["response"]["subtype"], "error");
        let stop = parse(interrupt_line("stop-1"));
        assert_eq!(stop["request"]["subtype"], "interrupt");
    }
}
