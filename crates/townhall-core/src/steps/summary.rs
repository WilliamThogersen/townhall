use std::path::Path;

use serde_json::Value;

const MAX_CHARS: usize = 80;
const ELLIPSIS: char = '\u{2026}';

/// A short label for a tool call: the file, command, pattern or description, else the tool name.
pub(super) fn summarize(tool: &str, input: Option<&Value>, cwd: Option<&str>) -> String {
    let field = |key: &str| {
        input
            .and_then(|i| i.get(key))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
    };
    let picked = match tool {
        "Read" | "Edit" | "MultiEdit" | "Write" => field("file_path").map(|p| relative(p, cwd)),
        "Bash" => field("command").map(clip),
        "Grep" | "Glob" => field("pattern").map(clip),
        "Agent" | "Task" => field("description").map(clip),
        _ => None,
    };
    picked.unwrap_or_else(|| tool.to_string())
}

/// `path` relative to `cwd` when it sits inside it.
fn relative(path: &str, cwd: Option<&str>) -> String {
    cwd.and_then(|c| Path::new(path).strip_prefix(c).ok())
        .map(|rel| rel.to_string_lossy().into_owned())
        .filter(|rel| !rel.is_empty())
        .unwrap_or_else(|| path.to_string())
}

/// The first line, at most [`MAX_CHARS`] chars, ending in an ellipsis when cut.
fn clip(text: &str) -> String {
    let line = text.lines().next().unwrap_or_default().trim();
    if line.chars().count() <= MAX_CHARS {
        return line.to_string();
    }
    let head: String = line.chars().take(MAX_CHARS - 1).collect();
    format!("{}{ELLIPSIS}", head.trim_end())
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::summarize;

    fn label(tool: &str, input: Value, cwd: Option<&str>) -> String {
        summarize(tool, Some(&input), cwd)
    }

    #[test]
    fn file_tools_show_the_path_relative_to_cwd() {
        let cwd = Some("/work/repo");
        for tool in ["Read", "Edit", "MultiEdit", "Write"] {
            let input = json!({ "file_path": "/work/repo/src/main.rs" });
            assert_eq!(label(tool, input, cwd), "src/main.rs");
        }
        let outside = json!({ "file_path": "/work/repository/x.rs" });
        assert_eq!(label("Read", outside, cwd), "/work/repository/x.rs");
        let no_cwd = json!({ "file_path": "/work/repo/a.rs" });
        assert_eq!(label("Write", no_cwd, None), "/work/repo/a.rs");
        let the_root = json!({ "file_path": "/work/repo" });
        assert_eq!(label("Read", the_root, cwd), "/work/repo");
    }

    #[test]
    fn bash_shows_the_first_line_trimmed_to_80_chars() {
        let multi = json!({ "command": "\n  git status  \ngit diff" });
        assert_eq!(label("Bash", multi, None), "git status");
        let long = format!("echo {}", "x".repeat(100));
        let clipped = label("Bash", json!({ "command": long }), None);
        assert_eq!(clipped.chars().count(), 80);
        assert!(clipped.starts_with("echo xxx"));
        assert!(clipped.ends_with('\u{2026}'));
        let exact = "y".repeat(80);
        assert_eq!(
            label("Bash", json!({ "command": exact.clone() }), None),
            exact
        );
    }

    #[test]
    fn search_and_agent_tools_show_pattern_and_description() {
        assert_eq!(
            label("Grep", json!({ "pattern": "fn main" }), None),
            "fn main"
        );
        assert_eq!(
            label("Glob", json!({ "pattern": "**/*.ts" }), None),
            "**/*.ts"
        );
        let agent = json!({ "description": "review the diff", "prompt": "long" });
        assert_eq!(label("Agent", agent, None), "review the diff");
        let task = json!({ "description": "explore" });
        assert_eq!(label("Task", task, None), "explore");
    }

    #[test]
    fn anything_else_falls_back_to_the_tool_name() {
        assert_eq!(
            label("WebFetch", json!({ "url": "https://x" }), None),
            "WebFetch"
        );
        assert_eq!(label("Bash", json!({ "command": "   " }), None), "Bash");
        assert_eq!(label("Read", json!({ "file_path": 7 }), None), "Read");
        assert_eq!(label("Grep", json!("not an object"), None), "Grep");
        assert_eq!(summarize("Bash", None, None), "Bash");
    }
}
