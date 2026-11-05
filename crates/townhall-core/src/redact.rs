//! The one path free-form text takes into a log line: secrets masked, length capped.

use std::borrow::Cow;

/// Longest text a log field may carry, in chars.
pub const MAX_CHARS: usize = 500;
const MASK: &str = "[REDACTED]";

/// Masks API keys, tokens, bearer credentials and `token=`/`password=`/`secret=` values,
/// then cuts the result to [`MAX_CHARS`] chars on a char boundary.
pub fn redact(text: &str) -> Cow<'_, str> {
    let masked = mask_assignments(&mask_tokens(text));
    let cut = match masked.char_indices().nth(MAX_CHARS) {
        Some((end, _)) => &masked[..end],
        None => &masked[..],
    };
    if cut == text {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(cut.to_owned())
    }
}

/// A credential shape: `prefix`, then body chars; the body is replaced by the mask.
struct Shape {
    prefix: &'static str,
    ignore_case: bool,
    /// The prefix stays in the output (`sk-ant-[REDACTED]`); otherwise it is masked too.
    keep_prefix: bool,
    /// Exact body length, or any non-empty run.
    len: Option<usize>,
    body: fn(char) -> bool,
}

const SHAPES: [Shape; 5] = [
    Shape {
        prefix: "sk-ant-",
        ignore_case: false,
        keep_prefix: true,
        len: None,
        body: |c| c.is_ascii_alphanumeric() || c == '-' || c == '_',
    },
    Shape {
        prefix: "ghp_",
        ignore_case: false,
        keep_prefix: true,
        len: None,
        body: |c| c.is_ascii_alphanumeric(),
    },
    Shape {
        prefix: "github_pat_",
        ignore_case: false,
        keep_prefix: true,
        len: None,
        body: |c| c.is_ascii_alphanumeric() || c == '_',
    },
    Shape {
        prefix: "Bearer ",
        ignore_case: true,
        keep_prefix: true,
        len: None,
        body: |c| !c.is_whitespace(),
    },
    Shape {
        prefix: "AKIA",
        ignore_case: false,
        keep_prefix: false,
        len: Some(16),
        body: |c| c.is_ascii_uppercase() || c.is_ascii_digit(),
    },
];

fn starts_with(rest: &str, prefix: &str, ignore_case: bool) -> bool {
    rest.get(..prefix.len()).is_some_and(|head| {
        if ignore_case {
            head.eq_ignore_ascii_case(prefix)
        } else {
            head == prefix
        }
    })
}

fn mask_tokens(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let mut prev: Option<char> = None;
    'scan: while i < text.len() {
        let rest = &text[i..];
        for shape in &SHAPES {
            if !starts_with(rest, shape.prefix, shape.ignore_case) {
                continue;
            }
            // `AKIA` glued to a word is not a key.
            if !shape.keep_prefix && prev.is_some_and(|c| c.is_ascii_alphanumeric()) {
                continue;
            }
            let after = &rest[shape.prefix.len()..];
            let body: usize = after
                .chars()
                .take_while(|c| (shape.body)(*c))
                .map(char::len_utf8)
                .sum();
            let matched = match shape.len {
                Some(n) => after.chars().take_while(|c| (shape.body)(*c)).count() >= n,
                None => body > 0,
            };
            if !matched {
                continue;
            }
            let body = shape.len.unwrap_or(body); // ASCII only, so chars == bytes
            if shape.keep_prefix {
                out.push_str(&rest[..shape.prefix.len()]);
            }
            out.push_str(MASK);
            i += shape.prefix.len() + body;
            prev = Some(']');
            continue 'scan;
        }
        let c = rest.chars().next().expect("i < len");
        out.push(c);
        prev = Some(c);
        i += c.len_utf8();
    }
    out
}

const KEYS: [&str; 3] = ["token=", "password=", "secret="];

/// Replaces the value of `token=`, `password=` and `secret=` (any case, any prefix such as
/// `access_token=`) up to the next space, quote, `&`, `;` or `,`.
fn mask_assignments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    'scan: while i < text.len() {
        let rest = &text[i..];
        for key in KEYS {
            if starts_with(rest, key, true) {
                let mut after = &rest[key.len()..];
                out.push_str(&rest[..key.len()]);
                // A quoted value keeps its opening quote; the closing one ends the value.
                if let Some(quote) = after.chars().next().filter(|c| matches!(c, '"' | '\'')) {
                    out.push(quote);
                    after = &after[1..];
                    i += 1;
                }
                let value = after
                    .find(|c: char| c.is_whitespace() || matches!(c, '&' | ';' | ',' | '"' | '\''))
                    .unwrap_or(after.len());
                if value > 0 {
                    out.push_str(MASK);
                }
                i += key.len() + value;
                continue 'scan;
            }
        }
        let c = rest.chars().next().expect("i < len");
        out.push(c);
        i += c.len_utf8();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_is_borrowed_untouched() {
        let out = redact("nothing secret here");
        assert!(matches!(out, Cow::Borrowed(_)));
        assert_eq!(out, "nothing secret here");
    }

    #[test]
    fn anthropic_keys_are_masked() {
        assert_eq!(
            redact("key sk-ant-api03-AbC_dEf-123 failed"),
            "key sk-ant-[REDACTED] failed"
        );
    }

    #[test]
    fn github_tokens_are_masked() {
        assert_eq!(
            redact("push with ghp_abcDEF123456 now"),
            "push with ghp_[REDACTED] now"
        );
        assert_eq!(
            redact("github_pat_11AAAB_xyz987 end"),
            "github_pat_[REDACTED] end"
        );
    }

    #[test]
    fn bearer_credentials_are_masked() {
        assert_eq!(
            redact("Authorization: Bearer abc.def.ghi\nnext"),
            "Authorization: Bearer [REDACTED]\nnext"
        );
        assert_eq!(redact("bearer xyz"), "bearer [REDACTED]");
    }

    #[test]
    fn aws_access_keys_are_masked() {
        assert_eq!(redact("id AKIAIOSFODNN7EXAMPLE used"), "id [REDACTED] used");
        // Too short, or glued to a word: not a key.
        assert_eq!(redact("AKIASHORT"), "AKIASHORT");
        assert_eq!(redact("xAKIAIOSFODNN7EXAMPLE"), "xAKIAIOSFODNN7EXAMPLE");
    }

    #[test]
    fn assigned_secrets_are_masked() {
        assert_eq!(redact("token=abc123 ok"), "token=[REDACTED] ok");
        assert_eq!(
            redact("https://h/x?password=hunter2&a=1"),
            "https://h/x?password=[REDACTED]&a=1"
        );
        assert_eq!(redact("SECRET=\"s3\" x"), "SECRET=\"[REDACTED]\" x");
        assert_eq!(redact("access_token=zzz"), "access_token=[REDACTED]");
        assert_eq!(redact("token= empty"), "token= empty");
    }

    #[test]
    fn long_text_is_cut_to_the_cap() {
        let long = "a".repeat(2_000);
        let out = redact(&long);
        assert_eq!(out.chars().count(), MAX_CHARS);
    }

    #[test]
    fn the_cut_respects_utf8_boundaries() {
        let long = "é".repeat(600);
        let out = redact(&long);
        assert_eq!(out.chars().count(), MAX_CHARS);
        assert!(out.chars().all(|c| c == 'é'));
        let exact = "日".repeat(MAX_CHARS);
        assert_eq!(redact(&exact), exact.as_str());
    }

    #[test]
    fn a_secret_is_masked_before_the_cut() {
        let text = format!("{}sk-ant-{}", "x".repeat(495), "k".repeat(50));
        let out = redact(&text);
        assert!(!out.contains("kkk"), "{out}");
        assert!(out.starts_with("xxxxx"));
    }
}
