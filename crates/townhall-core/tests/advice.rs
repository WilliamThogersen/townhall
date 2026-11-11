use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use townhall_core::definitions::parse_agent;
use townhall_core::runner::consult;

const SCRIPT: &str = r#"#!/bin/sh
dir='@DIR@'
n=$(( $(cat "$dir/count" 2>/dev/null || echo 0) + 1 ))
echo "$n" > "$dir/count"
printf '%s\0' "$@" > "$dir/argv-$n"
pwd -P > "$dir/cwd-$n"
IFS= read -r first || exit 4
printf '%s\n' "$first" > "$dir/stdin-$n"
echo '{"type":"system","subtype":"init","session_id":"fake","tools":[]}'
case "$first" in
  *SLEEP*) sleep 30 ;;
  *BOOM*)
    echo '{"type":"result","subtype":"error_during_execution","is_error":true,"result":"overloaded"}'
    exit 1 ;;
  *CRASH*) echo 'no auth' >&2; exit 3 ;;
  *EDIT*)
    echo '{"type":"control_request","request_id":"r-edit","request":{"subtype":"can_use_tool","tool_name":"Edit","input":{"file_path":"/x"},"tool_use_id":"t1"}}'
    IFS= read -r answer; printf '%s\n' "$answer" >> "$dir/stdin-$n"
    echo '{"type":"control_request","request_id":"r-read","request":{"subtype":"can_use_tool","tool_name":"Read","input":{"file_path":"/x"},"tool_use_id":"t2"}}'
    IFS= read -r answer; printf '%s\n' "$answer" >> "$dir/stdin-$n" ;;
esac
echo '{"type":"assistant","message":{"model":"claude-fable-5-2","role":"assistant","content":[{"type":"text","text":"let me look"}],"usage":{"input_tokens":10,"output_tokens":1}}}'
echo '{"type":"result","subtype":"success","is_error":false,"result":"use the second option","usage":{"input_tokens":1000,"output_tokens":34}}'
cat >> "$dir/stdin-$n"
"#;

const ADVISER: &str = "name = \"adviser\"\nadapter = \"claude\"\nmodel = \"claude-fable-5-2\"\neffort = \"high\"\npermission_mode = \"plan\"\ntools = [\"Read\", \"Grep\", \"Glob\", \"WebSearch\", \"WebFetch\"]\nprompt = \"be brief\"\n";

struct Fake {
    dir: PathBuf,
}

impl Fake {
    fn install(parent: &Path) -> Self {
        let dir = parent.join("fake-claude");
        std::fs::create_dir_all(&dir).unwrap();
        let dir = dir.canonicalize().unwrap();
        let script = dir.join("claude");
        std::fs::write(&script, SCRIPT.replace("@DIR@", &dir.to_string_lossy())).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        Self { dir }
    }

    fn bin(&self) -> String {
        self.dir.join("claude").to_string_lossy().into_owned()
    }

    fn argv(&self) -> Vec<String> {
        let text = std::fs::read_to_string(self.dir.join("argv-1")).unwrap();
        text.split_terminator('\0').map(String::from).collect()
    }

    fn stdin(&self) -> Vec<serde_json::Value> {
        let text = std::fs::read_to_string(self.dir.join("stdin-1")).unwrap();
        text.lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }
}

fn value_of(argv: &[String], flag: &str) -> Option<String> {
    let prefix = format!("{flag}=");
    argv.iter()
        .find_map(|a| a.strip_prefix(&prefix))
        .map(String::from)
}

struct Setup {
    _dir: tempfile::TempDir,
    fake: Fake,
    work: PathBuf,
}

fn setup() -> Setup {
    let dir = tempfile::TempDir::new().unwrap();
    let fake = Fake::install(dir.path());
    let work = dir.path().join("work");
    std::fs::create_dir_all(&work).unwrap();
    Setup {
        work: work.canonicalize().unwrap(),
        _dir: dir,
        fake,
    }
}

const LONG: Duration = Duration::from_secs(30);

#[tokio::test]
async fn the_adviser_runs_with_its_saved_model_effort_mode_tools_and_prompt() {
    let s = setup();
    let def = parse_agent(ADVISER).unwrap().def;
    let advice = consult(&s.fake.bin(), &def, &s.work, "which approach?", LONG)
        .await
        .unwrap();
    assert_eq!(advice.text, "use the second option");
    assert_eq!(advice.tokens, 1034);
    assert_eq!(advice.model, "claude-fable-5-2");
    let argv = s.fake.argv();
    assert_eq!(
        value_of(&argv, "--model").as_deref(),
        Some("claude-fable-5-2")
    );
    assert_eq!(value_of(&argv, "--effort").as_deref(), Some("high"));
    assert_eq!(
        value_of(&argv, "--permission-mode").as_deref(),
        Some("plan")
    );
    assert_eq!(
        value_of(&argv, "--tools").as_deref(),
        Some("Read,Grep,Glob,WebSearch,WebFetch")
    );
    assert_eq!(
        value_of(&argv, "--append-system-prompt").as_deref(),
        Some("be brief")
    );
    assert_eq!(
        value_of(&argv, "--disallowedTools").as_deref(),
        Some("Bash,Edit,MultiEdit,Write,NotebookEdit")
    );
    assert_eq!(
        value_of(&argv, "--mcp-config").as_deref(),
        Some("{\"mcpServers\":{}}")
    );
    assert!(argv.contains(&"--strict-mcp-config".to_string()));
    assert!(argv.contains(&"--no-session-persistence".to_string()));
    assert!(!argv.iter().any(|a| a.starts_with("--resume")));
    assert_eq!(
        std::fs::read_to_string(s.fake.dir.join("cwd-1"))
            .unwrap()
            .trim(),
        s.work.to_string_lossy()
    );
    let first = &s.fake.stdin()[0];
    assert!(first.to_string().contains("which approach?"), "{first}");
}

#[tokio::test]
async fn tools_that_write_or_reach_townhall_are_dropped_and_a_type_with_none_left_is_refused() {
    let s = setup();
    let sneaky = ADVISER.replace(
        "tools = [\"Read\", \"Grep\", \"Glob\", \"WebSearch\", \"WebFetch\"]",
        "tools = [\"Read\", \"Edit\", \"Bash(git *)\", \"Write\", \"mcp__townhall__create_task\", \"Grep\"]",
    );
    let def = parse_agent(&sneaky).unwrap().def;
    consult(&s.fake.bin(), &def, &s.work, "q", LONG)
        .await
        .unwrap();
    let argv = s.fake.argv();
    assert_eq!(value_of(&argv, "--tools").as_deref(), Some("Read,Grep"));
    assert_eq!(
        value_of(&argv, "--allowedTools").as_deref(),
        Some("Read,Grep")
    );

    let only_writes = ADVISER.replace(
        "tools = [\"Read\", \"Grep\", \"Glob\", \"WebSearch\", \"WebFetch\"]",
        "tools = [\"Edit\", \"Bash\"]",
    );
    let def = parse_agent(&only_writes).unwrap().def;
    let error = consult(&s.fake.bin(), &def, &s.work, "q", LONG)
        .await
        .unwrap_err();
    assert!(error.contains("read-only tools"), "{error}");
}

#[tokio::test]
async fn a_type_without_a_model_or_not_on_claude_is_refused_before_anything_runs() {
    let s = setup();
    let no_model = ADVISER.replace("model = \"claude-fable-5-2\"\n", "");
    let def = parse_agent(&no_model).unwrap().def;
    let error = consult(&s.fake.bin(), &def, &s.work, "q", LONG)
        .await
        .unwrap_err();
    assert!(error.contains("no model"), "{error}");
    let shell = parse_agent("name = \"adviser\"\nadapter = \"shell\"\ncommand = \"ls\"\n")
        .unwrap()
        .def;
    let error = consult(&s.fake.bin(), &shell, &s.work, "q", LONG)
        .await
        .unwrap_err();
    assert!(error.contains("claude"), "{error}");
    assert!(!s.fake.dir.join("count").exists());
}

#[tokio::test]
async fn a_tool_that_edits_is_denied_and_a_read_is_allowed() {
    let s = setup();
    let def = parse_agent(ADVISER).unwrap().def;
    consult(&s.fake.bin(), &def, &s.work, "EDIT please", LONG)
        .await
        .unwrap();
    let lines = s.fake.stdin();
    let behavior = |id: &str| {
        lines
            .iter()
            .find(|l| l["response"]["request_id"] == id)
            .map(|l| l["response"]["response"]["behavior"].clone())
    };
    assert_eq!(behavior("r-edit"), Some("deny".into()));
    assert_eq!(behavior("r-read"), Some("allow".into()));
}

#[tokio::test]
async fn a_failed_run_says_why() {
    let s = setup();
    let def = parse_agent(ADVISER).unwrap().def;
    let bin = s.fake.bin();
    let error = consult(&bin, &def, &s.work, "BOOM", LONG)
        .await
        .unwrap_err();
    assert_eq!(error, "claude failed: overloaded");
    let error = consult(&bin, &def, &s.work, "CRASH", LONG)
        .await
        .unwrap_err();
    assert!(error.contains("no auth"), "{error}");
    let gone = s.work.join("gone");
    let error = consult(&bin, &def, &gone, "q", LONG).await.unwrap_err();
    assert!(error.contains("folder"), "{error}");
}

#[tokio::test]
async fn a_run_that_takes_too_long_is_killed_with_a_clear_error() {
    let s = setup();
    let def = parse_agent(ADVISER).unwrap().def;
    let started = std::time::Instant::now();
    let error = consult(
        &s.fake.bin(),
        &def,
        &s.work,
        "SLEEP",
        Duration::from_millis(600),
    )
    .await
    .unwrap_err();
    assert!(error.contains("did not answer within"), "{error}");
    assert!(started.elapsed() < Duration::from_secs(20));
}
