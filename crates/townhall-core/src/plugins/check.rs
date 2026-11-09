use crate::{Error, Result};

const MAX_NAME_LEN: usize = 64;

pub(super) fn invalid(what: &str, why: impl std::fmt::Display) -> Error {
    Error::InvalidInput(format!("{what}: {why}"))
}

pub(super) fn size(what: &str, text: &str, max: usize) -> Result<()> {
    if text.len() > max {
        return Err(Error::TooLarge(format!(
            "{what} is {} bytes, the limit is {max}",
            text.len()
        )));
    }
    Ok(())
}

pub(super) fn name(what: &str, value: &str) -> Result<()> {
    let ok = value.len() <= MAX_NAME_LEN
        && value.starts_with(|c: char| c.is_ascii_alphanumeric())
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'));
    if ok {
        Ok(())
    } else {
        Err(invalid(
            what,
            format!(
                "`{value}` is not valid: use up to {MAX_NAME_LEN} letters, digits, `.`, `-` and `_`, starting with a letter or digit"
            ),
        ))
    }
}

pub(super) fn word(what: &str, value: &str, extra: &[char]) -> Result<()> {
    let ok = !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || extra.contains(&c));
    if ok {
        Ok(())
    } else {
        Err(invalid(
            what,
            format!("`{value}` is not valid: use letters, digits and {extra:?} only"),
        ))
    }
}

pub(super) fn env_name(what: &str, value: &str) -> Result<()> {
    let mut chars = value.chars();
    let ok = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
    if ok {
        Ok(())
    } else {
        Err(invalid(
            what,
            format!("`{value}` is not an environment variable name"),
        ))
    }
}

pub(super) fn one_of(what: &str, value: &str, allowed: &[&str]) -> Result<()> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(invalid(
            what,
            format!("`{value}` is unknown, use one of {}", allowed.join(", ")),
        ))
    }
}

pub(super) fn rel_path(what: &str, value: &str) -> Result<()> {
    if value.is_empty() {
        return Err(invalid(what, "the path is empty"));
    }
    let drive = value.as_bytes().get(1) == Some(&b':');
    if value.starts_with('/')
        || drive
        || value.contains('\\')
        || value.chars().any(|c| c.is_control())
    {
        return Err(invalid(
            what,
            format!("`{value}` must be a relative path with `/` separators, not an absolute one"),
        ));
    }
    if value.split('/').any(|part| part == "..") {
        return Err(invalid(
            what,
            format!("`{value}` must stay inside the plugin: `..` is not allowed"),
        ));
    }
    Ok(())
}

pub(super) fn semver(what: &str, value: &str) -> Result<()> {
    let bad = || {
        invalid(
            what,
            format!("`{value}` is not a semantic version like 1.2.3"),
        )
    };
    let (rest, _build) = value.split_once('+').unwrap_or((value, ""));
    let (core, pre) = rest.split_once('-').unwrap_or((rest, ""));
    let numbers: Vec<&str> = core.split('.').collect();
    let numeric = |part: &str| {
        !part.is_empty()
            && part.bytes().all(|b| b.is_ascii_digit())
            && (part == "0" || !part.starts_with('0'))
    };
    let ident = |part: &str| {
        !part.is_empty() && part.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    };
    let build_ok = value
        .split_once('+')
        .is_none_or(|(_, build)| build.split('.').all(ident));
    let pre_ok = rest.split_once('-').is_none() || pre.split('.').all(ident);
    if numbers.len() == 3 && numbers.iter().all(|n| numeric(n)) && pre_ok && build_ok {
        Ok(())
    } else {
        Err(bad())
    }
}

pub(super) fn http_url(what: &str, value: &str) -> Result<()> {
    let rest = value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://"));
    match rest {
        Some(rest) if !rest.is_empty() && !has_space(value) => Ok(()),
        _ => Err(invalid(
            what,
            format!("`{value}` is not an http or https address"),
        )),
    }
}

pub(super) fn git_url(what: &str, value: &str) -> Result<()> {
    let https = value
        .strip_prefix("https://")
        .is_some_and(|rest| !rest.is_empty());
    let scp = value
        .strip_prefix("git@")
        .and_then(|rest| rest.split_once(':'))
        .is_some_and(|(host, path)| {
            !host.is_empty()
                && host
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'))
                && !path.is_empty()
        });
    if (https || scp) && !has_space(value) {
        Ok(())
    } else {
        Err(invalid(
            what,
            format!("`{value}` must be an https url or a `git@host:owner/repo.git` address"),
        ))
    }
}

pub(super) fn github_repo(what: &str, value: &str) -> Result<()> {
    let part = |p: &str| {
        !p.is_empty()
            && p != "."
            && p != ".."
            && p.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
    };
    match value.split_once('/') {
        Some((owner, repo)) if part(owner) && part(repo) => Ok(()),
        _ => Err(invalid(
            what,
            format!("`{value}` must look like `owner/name`"),
        )),
    }
}

pub(super) fn sha(what: &str, value: &str) -> Result<()> {
    if value.len() == 40 && value.bytes().all(|b| b.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(invalid(what, format!("`{value}` is not 40 hex characters")))
    }
}

pub(super) fn git_ref(what: &str, value: &str) -> Result<()> {
    let ok = !value.is_empty()
        && !value.starts_with('-')
        && !value.contains("..")
        && !has_space(value)
        && !value.contains(['~', '^', ':', '?', '*', '[', '\\']);
    if ok {
        Ok(())
    } else {
        Err(invalid(
            what,
            format!("`{value}` is not a usable branch, tag or ref"),
        ))
    }
}

fn has_space(value: &str) -> bool {
    value.chars().any(|c| c.is_whitespace() || c.is_control())
}
