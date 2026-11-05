//! A stable fingerprint for a log message: the same failure with other numbers, ids, paths
//! or names gets the same fingerprint, so recurring errors group.

/// 16 hex chars: FNV-1a 64 of `target`, `event` and the normalized `message`. FNV (not
/// `DefaultHasher`) because the value goes into task briefs and must not change between builds.
/// `ids` are the entry's id fields (`task`, `agent`, ...): their values are placeholders even
/// when they look like plain words.
pub fn fingerprint(target: &str, event: &str, message: &str, ids: &[(&str, &str)]) -> String {
    let normalized = normalize(message, ids);
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for part in [target, "\0", event, "\0", &normalized] {
        for byte in part.bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
    }
    format!("{hash:016x}")
}

/// The message with ids, quoted text, uuids, hex, paths and numbers as placeholders.
pub fn normalize(message: &str, ids: &[(&str, &str)]) -> String {
    let mut text = message.to_string();
    for (_, value) in ids {
        if value.len() >= 2 {
            text = text.replace(value, "<name>");
        }
    }
    replace_quoted(&text)
        .split_whitespace()
        .map(word)
        .collect::<Vec<_>>()
        .join(" ")
}

/// `'x'`, `"x"` and `` `x` `` become `<q>`. A `'` inside a word (`can't`) is not a quote.
fn replace_quoted(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let opens = matches!(c, '"' | '`' | '\'')
            && (i == 0 || !chars[i - 1].is_alphanumeric())
            && chars.get(i + 1).is_some_and(|n| !n.is_whitespace());
        if opens {
            let close = (i + 1..chars.len()).find(|&j| {
                chars[j] == c
                    && (c != '\'' || chars.get(j + 1).is_none_or(|n| !n.is_alphanumeric()))
            });
            if let Some(j) = close {
                out.push_str("<q>");
                i = j + 1;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

const EDGE: &[char] = &[',', ';', ':', '(', ')', '[', ']', '{', '}', '.', '!', '?'];

fn word(w: &str) -> String {
    let core = w.trim_matches(EDGE);
    if core.is_empty() {
        return w.to_string();
    }
    let start = w.find(core).unwrap_or(0);
    let (head, tail) = (&w[..start], &w[start + core.len()..]);
    format!("{head}{}{tail}", core_word(core))
}

fn core_word(w: &str) -> String {
    if is_path(w) {
        return "<path>".into();
    }
    if is_uuid(w) {
        return "<uuid>".into();
    }
    if is_hex(w) {
        return "<hex>".into();
    }
    let digits = replace_digits(w);
    if is_numbered_name(&digits) {
        return "<name>".into();
    }
    digits
}

fn is_path(w: &str) -> bool {
    ["/", "./", "../", "~/"].iter().any(|p| w.starts_with(p)) && w.len() > 1
}

fn is_uuid(w: &str) -> bool {
    w.len() == 36
        && w.char_indices().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => c == '-',
            _ => c.is_ascii_hexdigit(),
        })
}

/// `0x1f`, or 7+ hex digits with at least one decimal digit (a short git sha).
fn is_hex(w: &str) -> bool {
    if let Some(rest) = w.strip_prefix("0x") {
        return !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_hexdigit());
    }
    w.len() >= 7
        && w.bytes().all(|b| b.is_ascii_hexdigit())
        && w.bytes().any(|b| b.is_ascii_digit())
}

/// Digit runs become `<n>`: `312ms` is `<n>ms`.
fn replace_digits(w: &str) -> String {
    let mut out = String::with_capacity(w.len());
    let mut in_run = false;
    for c in w.chars() {
        if c.is_ascii_digit() {
            if !in_run {
                out.push_str("<n>");
            }
            in_run = true;
        } else {
            in_run = false;
            out.push(c);
        }
    }
    out
}

/// A kebab or snake name with a numeric suffix (`foo-<n>`): a task name like `fix-login-2`.
fn is_numbered_name(w: &str) -> bool {
    let Some(base) = w.strip_suffix("-<n>").or_else(|| w.strip_suffix("_<n>")) else {
        return false;
    };
    !base.is_empty()
        && base.starts_with(|c: char| c.is_ascii_alphabetic())
        && base
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}
