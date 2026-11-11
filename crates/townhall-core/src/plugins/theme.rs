use super::check;
use crate::Result;

pub const MAX_THEME_BYTES: usize = 64 * 1024;

const WHAT: &str = "contributes.theme";
const PREFIX: &str = "--th-";
const SELECTORS: [&str; 3] = [":root", "[data-theme=\"dark\"]", "[data-theme=\"light\"]"];
const FUNCTIONS: [&str; 20] = [
    "rgb",
    "rgba",
    "hsl",
    "hsla",
    "hwb",
    "lab",
    "lch",
    "oklab",
    "oklch",
    "color",
    "color-mix",
    "var",
    "calc",
    "min",
    "max",
    "clamp",
    "sin",
    "cos",
    "round",
    "abs",
];
const VALUE_PUNCTUATION: &str = " #%.,()-_/+*\"'";

pub fn parse(text: &str) -> Result<String> {
    check::size(WHAT, text, MAX_THEME_BYTES)?;
    if text.contains('\\') {
        return Err(check::invalid(
            WHAT,
            "a backslash is not allowed: escapes could hide a url or a property name",
        ));
    }
    let clean = strip_comments(text)?;
    let mut rest = clean.trim();
    let mut out = String::new();
    let mut declared = 0;
    while !rest.is_empty() {
        let (prelude, after) = prelude(rest)?;
        let (body, next) = body(after)?;
        let selectors = selectors(prelude)?;
        let declarations = declarations(body)?;
        declared += declarations.len();
        out.push_str(&selectors.join(", "));
        out.push_str(" {\n");
        for (name, value) in declarations {
            out.push_str(&format!("  {name}: {value};\n"));
        }
        out.push_str("}\n");
        rest = next.trim_start();
    }
    if out.is_empty() {
        return Err(check::invalid(WHAT, "the theme has no rules"));
    }
    if declared == 0 {
        return Err(check::invalid(WHAT, "the theme sets no variables"));
    }
    Ok(out)
}

fn strip_comments(text: &str) -> Result<String> {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut quote: Option<char> = None;
    while let Some(c) = chars.next() {
        match quote {
            Some(open) => {
                if c == '\n' {
                    return Err(check::invalid(WHAT, "a string runs over a line break"));
                }
                if c == open {
                    quote = None;
                }
                out.push(c);
            }
            None if c == '"' || c == '\'' => {
                quote = Some(c);
                out.push(c);
            }
            None if c == '/' && chars.peek() == Some(&'*') => {
                chars.next();
                let mut closed = false;
                while let Some(inner) = chars.next() {
                    if inner == '*' && chars.peek() == Some(&'/') {
                        chars.next();
                        closed = true;
                        break;
                    }
                }
                if !closed {
                    return Err(check::invalid(WHAT, "a comment is never closed"));
                }
                out.push(' ');
            }
            None => out.push(c),
        }
    }
    if quote.is_some() {
        return Err(check::invalid(WHAT, "a string is never closed"));
    }
    Ok(out)
}

fn find(text: &str, stops: &[char]) -> Option<(usize, char)> {
    let mut quote: Option<char> = None;
    for (at, c) in text.char_indices() {
        match quote {
            Some(open) if c == open => quote = None,
            Some(_) => {}
            None if c == '"' || c == '\'' => quote = Some(c),
            None if stops.contains(&c) => return Some((at, c)),
            None => {}
        }
    }
    None
}

fn prelude(rest: &str) -> Result<(&str, &str)> {
    if rest.starts_with('@') {
        let name: String = rest
            .chars()
            .take_while(|c| *c == '@' || c.is_ascii_alphanumeric() || *c == '-')
            .collect();
        return Err(check::invalid(
            WHAT,
            format!("`{name}` is not allowed: at-rules such as @import are refused"),
        ));
    }
    match find(rest, &['{', ';', '}']) {
        Some((at, '{')) => Ok((rest[..at].trim(), &rest[at + 1..])),
        Some((_, other)) => Err(check::invalid(
            WHAT,
            format!("found `{other}` where a selector and `{{` should be"),
        )),
        None => Err(check::invalid(WHAT, "a `{` is missing after a selector")),
    }
}

fn body(after: &str) -> Result<(&str, &str)> {
    match find(after, &['{', '}']) {
        Some((at, '}')) => Ok((&after[..at], &after[at + 1..])),
        Some(_) => Err(check::invalid(
            WHAT,
            "a nested rule is not allowed: a rule holds only --th-* declarations",
        )),
        None => Err(check::invalid(WHAT, "a rule has no closing `}`")),
    }
}

fn selectors(prelude: &str) -> Result<Vec<&str>> {
    prelude
        .split(',')
        .map(str::trim)
        .map(|selector| {
            if SELECTORS.contains(&selector) {
                Ok(selector)
            } else {
                Err(check::invalid(
                    WHAT,
                    format!(
                        "selector `{selector}` is not allowed: use :root, [data-theme=\"dark\"] or [data-theme=\"light\"]"
                    ),
                ))
            }
        })
        .collect()
}

fn declarations(body: &str) -> Result<Vec<(&str, String)>> {
    let mut found = Vec::new();
    let mut rest = body;
    loop {
        let (piece, next) = match find(rest, &[';']) {
            Some((at, _)) => (&rest[..at], Some(&rest[at + 1..])),
            None => (rest, None),
        };
        let piece = piece.trim();
        if !piece.is_empty() {
            found.push(declaration(piece)?);
        }
        match next {
            Some(next) => rest = next,
            None => return Ok(found),
        }
    }
}

fn declaration(piece: &str) -> Result<(&str, String)> {
    let Some((name, value)) = piece.split_once(':') else {
        return Err(check::invalid(
            WHAT,
            format!("`{piece}` needs a `:` between the name and the value"),
        ));
    };
    let name = name.trim();
    let tail = name.strip_prefix(PREFIX).unwrap_or_default();
    let named = !tail.is_empty()
        && tail
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if !named {
        return Err(check::invalid(
            WHAT,
            format!("property `{name}` is not allowed: only --th-* custom properties (lowercase letters, digits and `-`)"),
        ));
    }
    Ok((name, value_of(name, value.trim())?))
}

fn value_of(name: &str, value: &str) -> Result<String> {
    if value.is_empty() {
        return Err(check::invalid(WHAT, format!("`{name}` has no value")));
    }
    let lower = value.to_ascii_lowercase();
    if lower.contains("url(") {
        return Err(check::invalid(
            WHAT,
            format!("`{name}`: url( is not allowed, a theme loads nothing"),
        ));
    }
    if lower.contains("expression") {
        return Err(check::invalid(
            WHAT,
            format!("`{name}`: expression is not allowed"),
        ));
    }
    let mut quote: Option<char> = None;
    let mut word = String::new();
    let mut depth = 0usize;
    for c in value.chars() {
        if let Some(open) = quote {
            if c.is_control() {
                return Err(check::invalid(
                    WHAT,
                    format!("`{name}`: a string holds a control character"),
                ));
            }
            if c == open {
                quote = None;
            }
            continue;
        }
        match c {
            '"' | '\'' => {
                quote = Some(c);
                word.clear();
            }
            '(' => {
                depth += 1;
                let function = word.to_ascii_lowercase();
                if !function.is_empty() && !FUNCTIONS.contains(&function.as_str()) {
                    return Err(check::invalid(
                        WHAT,
                        format!("`{name}`: function `{function}(` is not allowed"),
                    ));
                }
                word.clear();
            }
            ')' => {
                depth = depth.checked_sub(1).ok_or_else(|| {
                    check::invalid(WHAT, format!("`{name}`: unbalanced parentheses"))
                })?;
                word.clear();
            }
            c if c.is_ascii_alphanumeric() || c == '-' || c == '_' => word.push(c),
            c if VALUE_PUNCTUATION.contains(c) => word.clear(),
            other => {
                return Err(check::invalid(
                    WHAT,
                    format!(
                        "`{name}`: the character `{}` is not allowed in a value",
                        other.escape_debug()
                    ),
                ))
            }
        }
    }
    if depth != 0 {
        return Err(check::invalid(
            WHAT,
            format!("`{name}`: unbalanced parentheses"),
        ));
    }
    Ok(value.split_whitespace().collect::<Vec<_>>().join(" "))
}

#[cfg(test)]
mod tests;
