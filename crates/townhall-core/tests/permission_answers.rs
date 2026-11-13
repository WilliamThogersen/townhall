use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use tempfile::TempDir;
use townhall_core::agent::watch::Watcher;
use townhall_core::agent::{self, shell_quote, LaunchOptions, ShellAdapter};
use townhall_core::model::{EntryBody, NewTask, PermissionVerdict, SettingKey, Task, TaskState};
use townhall_core::{tmux, Context, ContextOptions, Home, Workspace};

mod common;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

const FAKE_CLAUDE: &str = r#"#!/bin/sh
dir='@DIR@'
n=$(( $(cat "$dir/count" 2>/dev/null || echo 0) + 1 ))
echo "$n" > "$dir/count"
IFS= read -r first || exit 4
printf '%s\n' "$first" > "$dir/stdin-$n"
echo '{"type":"system","subtype":"init","session_id":"fake","tools":[]}'
case "$first" in
  *"git status"*) text='ALLOW: a read-only git command in its own worktree' ;;
  *"cargo publish"*) text='DENY: publishing is not part of this task, run cargo package instead' ;;
  *"npm publish"*) text='ASK USER: it publishes to a registry' ;;
  *"sleep 99"*) sleep 30; text='ALLOW: too late' ;;
  *) text='ASK USER: not sure' ;;
esac
printf '{"type":"result","subtype":"success","is_error":false,"result":"%s","usage":{"input_tokens":1,"output_tokens":1}}\n' "$text"
cat >> "$dir/stdin-$n"
"#;

const ADVISER: &str = "name = \"adviser\"\nadapter = \"claude\"\nmodel = \"claude-fable-5-2\"\npermission_mode = \"plan\"\ntools = [\"Read\", \"Grep\", \"Glob\"]\nprompt = \"be brief\"\n";

fn screen_for(command: &str) -> String {
    format!(
        " Bash command\n\n   {command}\n   Run it\n\n Do you want to proceed?\n \u{276f} 1. Yes\n   2. Yes, and don't ask again for this command\n   3. No, and tell Claude what to do differently (esc)\n"
    )
}

struct Sessions(Vec<String>);

impl Drop for Sessions {
    fn drop(&mut self) {
        for session in &self.0 {
            let target = format!("={session}");
            let _ = common::tmux()
                .args(["kill-session", "-t", &target])
                .output();
        }
    }
}

struct Env {
    _repo: TempDir,
    _home: TempDir,
    ctx: Context,
    repo_name: String,
    conversation: String,
    dir: TempDir,
    sessions: Sessions,
}

async fn env() -> Env {
    let repo = common::init_repo().await;
    let home = TempDir::new().unwrap();
    let dir = TempDir::new().unwrap();
    let fake = dir.path().join("claude");
    let script = FAKE_CLAUDE.replace("@DIR@", &dir.path().to_string_lossy());
    std::fs::write(&fake, script).unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    let ws = Workspace::new(Home::at(home.path().join(".townhall")), repo.path()).unwrap();
    let options = ContextOptions {
        claude_bin: fake.to_string_lossy().into_owned(),
        ..ContextOptions::default()
    };
    let ctx = Context::open(ws, options).unwrap();
    let repo_name = ctx.ensure_startup_repo().await.unwrap();
    let store = ctx.store().unwrap();
    let conversation = store.create_conversation().await.unwrap().id;
    store.save_agent_type(ADVISER).await.unwrap();
    Env {
        _repo: repo,
        _home: home,
        ctx,
        repo_name,
        conversation,
        dir,
        sessions: Sessions(vec![]),
    }
}

impl Env {
    fn keys_file(&self, name: &str) -> PathBuf {
        self.dir.path().join(format!("keys-{name}"))
    }

    fn adviser_runs(&self) -> usize {
        std::fs::read_to_string(self.dir.path().join("count"))
            .map(|s| s.trim().parse().unwrap())
            .unwrap_or(0)
    }

    fn keys(&self, name: &str) -> String {
        std::fs::read_to_string(self.keys_file(name)).unwrap_or_default()
    }

    async fn agent(&mut self, command: &str, agent_type: &str) -> String {
        let name = format!(
            "p-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        );
        let keys = shell_quote(&self.keys_file(&name).to_string_lossy());
        let show = format!("printf '%s' {}", shell_quote(&screen_for(command)));
        let clear = "printf '\\033[2J\\033[Hwelcome\\n'";
        let script = format!(
            "stty -echo -icanon min 1; {show}; while :; do k=$(dd bs=1 count=1 2>/dev/null); if [ -z \"$k\" ]; then printf '<nl>' >> {keys}; else printf '%s' \"$k\" >> {keys}; fi; {clear}; done"
        );
        let ws = self.ctx.startup().clone();
        self.sessions.0.push(tmux::session_name(ws.key(), &name));
        let options = LaunchOptions {
            command: Some(format!("sh -c {}", shell_quote(&script))),
            ..LaunchOptions::default()
        };
        agent::start(&ws, &name, "main", &ShellAdapter, &options)
            .await
            .unwrap();
        let store = self.ctx.store().unwrap();
        let new = NewTask {
            name: name.clone(),
            title: format!("do {name}"),
            prompt: "publish nothing, only inspect the repo".into(),
            repo: self.repo_name.clone(),
            base_branch: "main".into(),
            after: vec![],
            attachments: vec![],
            agent: agent_type.into(),
            pr_base: None,
            done: Default::default(),
        };
        store.create_task(&self.conversation, new).await.unwrap();
        let working = |t: Task| {
            Ok(Task {
                state: TaskState::Working,
                ..t
            })
        };
        store.update_task(&name, working).await.unwrap();
        name
    }

    async fn needs_you(&self, name: &str) -> Option<(String, Option<String>)> {
        let entries = self.ctx.store().unwrap().entries(&self.conversation).await;
        entries.unwrap().into_iter().find_map(|e| match e.body {
            EntryBody::NeedsYou {
                agent,
                question,
                advice,
                ..
            } if agent == name => Some((question, advice)),
            _ => None,
        })
    }

    async fn poll_until<F: Fn(&Env) -> bool>(&self, watcher: &mut Watcher, seconds: u64, done: F) {
        let deadline = Instant::now() + Duration::from_secs(seconds);
        while !done(self) {
            assert!(Instant::now() < deadline, "the watcher never got there");
            watcher.poll().await;
            tokio::time::sleep(Duration::from_millis(150)).await;
        }
        for _ in 0..4 {
            tokio::time::sleep(Duration::from_millis(150)).await;
            watcher.poll().await;
        }
    }

    async fn needs_you_shown(&self, watcher: &mut Watcher, name: &str, seconds: u64) {
        let deadline = Instant::now() + Duration::from_secs(seconds);
        while self.needs_you(name).await.is_none() {
            assert!(Instant::now() < deadline, "no needs-you entry appeared");
            watcher.poll().await;
            tokio::time::sleep(Duration::from_millis(150)).await;
        }
    }

    async fn decisions(&self) -> Vec<(PermissionVerdict, String)> {
        let log = self.ctx.store().unwrap().permission_decisions(20).await;
        let log = log.unwrap();
        log.into_iter().map(|d| (d.verdict, d.reason)).collect()
    }
}

#[tokio::test]
async fn an_allowed_command_gets_yes_and_never_reaches_the_user() {
    let mut env = env().await;
    let name = env.agent("git status", "builder").await;
    let mut watcher = Watcher::new(env.ctx.clone()).await;
    env.poll_until(&mut watcher, 15, |e| !e.keys(&name).is_empty())
        .await;
    assert_eq!(env.keys(&name), "1", "option 1, not `don't ask again`");
    assert_eq!(env.needs_you(&name).await, None);
    assert_eq!(env.adviser_runs(), 1);
    let log = env.decisions().await;
    assert_eq!(log.len(), 1, "{log:?}");
    assert_eq!(log[0].0, PermissionVerdict::Allow);
    assert!(log[0].1.contains("read-only git command"), "{log:?}");
    agent::stop(env.ctx.startup(), &name).await.unwrap();
}

#[tokio::test]
async fn a_denied_command_gets_no_and_the_reason_in_one_line() {
    let mut env = env().await;
    let name = env.agent("cargo publish", "builder").await;
    let mut watcher = Watcher::new(env.ctx.clone()).await;
    env.poll_until(&mut watcher, 15, |e| e.keys(&name).contains("<nl>"))
        .await;
    let keys = env.keys(&name);
    assert!(keys.starts_with('3'), "option 3, `No`: {keys}");
    assert!(
        keys.contains("publishing is not part of this task"),
        "the agent is told why: {keys}"
    );
    assert_eq!(keys.matches("<nl>").count(), 1, "one line: {keys}");
    assert_eq!(env.needs_you(&name).await, None);
    assert_eq!(env.decisions().await[0].0, PermissionVerdict::Deny);
    agent::stop(env.ctx.startup(), &name).await.unwrap();
}

#[tokio::test]
async fn an_ask_user_verdict_reaches_the_user_with_the_reasoning() {
    let mut env = env().await;
    let name = env.agent("npm publish", "builder").await;
    let mut watcher = Watcher::new(env.ctx.clone()).await;
    env.needs_you_shown(&mut watcher, &name, 15).await;
    let (question, advice) = env.needs_you(&name).await.unwrap();
    assert!(
        question.starts_with("Do you want to proceed?"),
        "{question}"
    );
    assert_eq!(advice.as_deref(), Some("it publishes to a registry"));
    assert_eq!(env.keys(&name), "", "nothing was pressed");
    assert_eq!(env.decisions().await[0].0, PermissionVerdict::AskUser);
    let task = env.ctx.store().unwrap().find_task(&name).await.unwrap();
    assert_eq!(task.unwrap().state, TaskState::NeedsYou);
    agent::stop(env.ctx.startup(), &name).await.unwrap();
}

#[tokio::test]
async fn the_hard_deny_list_goes_to_the_user_without_asking_the_adviser() {
    let mut env = env().await;
    for command in [
        "git push --force origin main",
        "rm -rf /Users/me/code",
        "cat ~/.ssh/id_rsa",
        "psql -c 'drop database shop'",
    ] {
        let name = env.agent(command, "builder").await;
        let mut watcher = Watcher::new(env.ctx.clone()).await;
        env.needs_you_shown(&mut watcher, &name, 15).await;
        let (_, advice) = env.needs_you(&name).await.unwrap();
        let advice = advice.unwrap_or_default();
        assert!(
            advice.starts_with("always yours to decide"),
            "{command}: {advice}"
        );
        assert_eq!(env.keys(&name), "", "{command}: nothing was pressed");
        agent::stop(env.ctx.startup(), &name).await.unwrap();
    }
    assert_eq!(env.adviser_runs(), 0, "the adviser was never asked");
    let log = env.decisions().await;
    assert_eq!(log.len(), 4);
    assert!(log.iter().all(|(v, _)| *v == PermissionVerdict::HardDeny));
}

#[tokio::test]
async fn a_type_in_plan_mode_is_never_granted_more_than_that() {
    let mut env = env().await;
    let source = "name = \"scout\"\nadapter = \"claude\"\npermission_mode = \"plan\"\n";
    env.ctx
        .store()
        .unwrap()
        .save_agent_type(source)
        .await
        .unwrap();
    let name = env.agent("git status", "scout").await;
    let mut watcher = Watcher::new(env.ctx.clone()).await;
    env.needs_you_shown(&mut watcher, &name, 15).await;
    let (_, advice) = env.needs_you(&name).await.unwrap();
    assert!(advice.unwrap().contains("plan mode"));
    assert_eq!(env.keys(&name), "");
    agent::stop(env.ctx.startup(), &name).await.unwrap();
}

#[tokio::test]
async fn an_adviser_that_does_not_answer_in_time_falls_back_to_the_user() {
    let mut env = env().await;
    let store = env.ctx.store().unwrap().clone();
    store
        .set_setting(SettingKey::PermissionTimeoutSecs, 5.into())
        .await
        .unwrap();
    let name = env.agent("sleep 99", "builder").await;
    let mut watcher = Watcher::new(env.ctx.clone()).await;
    let started = Instant::now();
    env.needs_you_shown(&mut watcher, &name, 20).await;
    assert!(started.elapsed() >= Duration::from_secs(4), "it waited");
    let (_, advice) = env.needs_you(&name).await.unwrap();
    assert!(advice.unwrap().contains("did not answer within 5 s"));
    assert_eq!(env.keys(&name), "");
    assert_eq!(env.decisions().await[0].0, PermissionVerdict::Timeout);
    agent::stop(env.ctx.startup(), &name).await.unwrap();
}

#[tokio::test]
async fn with_the_setting_off_the_prompt_goes_to_the_user_as_before() {
    let mut env = env().await;
    let store = env.ctx.store().unwrap().clone();
    store
        .set_setting(SettingKey::AdviserPermissions, false.into())
        .await
        .unwrap();
    let name = env.agent("git status", "builder").await;
    let mut watcher = Watcher::new(env.ctx.clone()).await;
    env.needs_you_shown(&mut watcher, &name, 15).await;
    let (_, advice) = env.needs_you(&name).await.unwrap();
    assert_eq!(advice, None);
    assert_eq!(env.keys(&name), "");
    assert_eq!(env.adviser_runs(), 0);
    assert!(env.decisions().await.is_empty());
    agent::stop(env.ctx.startup(), &name).await.unwrap();
}
