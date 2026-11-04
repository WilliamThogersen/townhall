//! Rules for values that go into claude's argv as `--model=` and `--allowedTools=`
//! values. The adapter refuses a value that starts with `-`, or a tool with a comma,
//! too; checking here reports every such value when a definition is checked.

/// Why `model` cannot be the `claude --model=` value, if it cannot.
pub(crate) fn model_problem(model: &str) -> Option<String> {
    flag_problem(model)
        .or_else(|| {
            model
                .chars()
                .any(char::is_whitespace)
                .then_some("must not hold spaces")
        })
        .map(|problem| format!("\"{}\" {problem}", model.escape_debug()))
}

/// Why `tool` cannot be one `--allowedTools` value, if it cannot. Spaces may
/// only sit inside a rule's parentheses, as in `Bash(git diff:*)`. A comma never may:
/// the adapter joins the tools with commas, and claude splits them there.
pub(crate) fn tool_problem(tool: &str) -> Option<String> {
    flag_problem(tool)
        .or_else(|| {
            space_outside_parens(tool)
                .then_some("has a space outside parentheses: give each tool its own entry")
        })
        .or_else(|| {
            tool.contains(',')
                .then_some("must not hold a comma: give each tool its own entry")
        })
        .map(|problem| format!("entry \"{}\" {problem}", tool.escape_debug()))
}

fn flag_problem(value: &str) -> Option<&'static str> {
    if value.starts_with('-') {
        Some("must not start with \"-\"")
    } else if value.chars().any(char::is_control) {
        Some("must not hold control characters")
    } else {
        None
    }
}

fn space_outside_parens(value: &str) -> bool {
    value
        .chars()
        .try_fold(0usize, |depth, c| match c {
            '(' => Some(depth + 1),
            ')' => Some(depth.saturating_sub(1)),
            c if c.is_whitespace() && depth == 0 => None,
            _ => Some(depth),
        })
        .is_none()
}
