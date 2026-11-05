use super::agent::parse_agent;
use super::words::tool_problem;

pub fn add_tool(source: &str, tool: &str) -> Result<Option<String>, String> {
    let tool = tool.trim();
    if tool.is_empty() {
        return Err("the tool is empty".into());
    }
    if let Some(problem) = tool_problem(tool) {
        return Err(problem);
    }
    let parsed = parse_agent(source).map_err(|e| e.to_string())?.def;
    if parsed.tools.iter().any(|t| t == tool) {
        return Ok(None);
    }
    let mut want = parsed;
    want.tools.push(tool.to_string());
    let line = format!("tools = [{}]", quoted(&want.tools));
    let edited = match tools_span(source) {
        Some((start, end)) => format!("{}{}{}", &source[..start], line, &source[end..]),
        None if source.ends_with('\n') => format!("{source}{line}\n"),
        None => format!("{source}\n{line}\n"),
    };
    match parse_agent(&edited) {
        Ok(back) if back.def == want => Ok(Some(edited)),
        _ => Err("the tools list could not be edited safely: edit the type's TOML".into()),
    }
}

fn quoted(tools: &[String]) -> String {
    tools
        .iter()
        .map(|t| serde_json::to_string(t).unwrap_or_default())
        .collect::<Vec<_>>()
        .join(", ")
}

fn tools_span(source: &str) -> Option<(usize, usize)> {
    let mut offset = 0;
    let mut inside: Option<&str> = None;
    for line in source.split_inclusive('\n') {
        let open = inside;
        inside = block_after(line, inside);
        if open.is_some() {
            offset += line.len();
            continue;
        }
        let rest = line.trim_start();
        let after = rest.strip_prefix("tools").map(str::trim_start);
        if let Some(value) = after.and_then(|a| a.strip_prefix('=')) {
            let start = offset + (line.len() - rest.len());
            let value_at = offset + (line.len() - value.len());
            return array_end(source, value_at).map(|end| (start, end));
        }
        offset += line.len();
    }
    None
}

fn block_after<'a>(line: &str, open: Option<&'a str>) -> Option<&'a str> {
    ["\"\"\"", "'''"]
        .iter()
        .fold(open, |open, fence| match open {
            Some(f) if f == *fence => (line.matches(fence).count() % 2 == 0).then_some(f),
            None if line.matches(fence).count() % 2 == 1 => Some(fence),
            other => other,
        })
}

fn array_end(source: &str, from: usize) -> Option<usize> {
    let (mut depth, mut seen) = (0usize, false);
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut comment = false;
    for (i, c) in source[from..].char_indices() {
        if comment {
            comment = c != '\n';
            continue;
        }
        if let Some(q) = quote {
            if escaped {
                escaped = false;
            } else if c == '\\' && q == '"' {
                escaped = true;
            } else if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '"' | '\'' => quote = Some(c),
            '#' => comment = true,
            '[' => {
                depth += 1;
                seen = true;
            }
            ']' => {
                depth = depth.checked_sub(1)?;
                if seen && depth == 0 {
                    return Some(from + i + 1);
                }
            }
            _ => {}
        }
    }
    None
}
