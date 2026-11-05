use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to run `{program}`: {source}")]
    Spawn {
        program: String,
        #[source]
        source: std::io::Error,
    },
    /// Shown as its stderr first and the command last, so a message cut short (a task's
    /// `could not start`) keeps the reason and loses the long command line.
    #[error("{}", command_failed(.command, *.code, .stderr))]
    Command {
        command: String,
        code: Option<i32>,
        stderr: String,
    },
    #[error("{context}: {source}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid name `{0}`: use letters, digits, `-` and `_` only")]
    InvalidName(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("agent `{0}` not found")]
    NotFound(String),
    /// Anything other than an agent that does not exist: a repo, conversation, task or attachment.
    #[error("{what} `{name}` not found")]
    Missing { what: &'static str, name: String },
    /// The request is valid but clashes with what exists (a taken name, an ambiguous agent).
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    TooLarge(String),
    /// A database call failed.
    #[error("database: {0}")]
    Database(String),
    /// The database cannot be used: another townhall holds it, or it is from a newer townhall.
    #[error("{0}")]
    Unavailable(String),
    #[error("corrupt file {path}: {message}")]
    Corrupt { path: String, message: String },
    #[error("{cause}; rollback also failed: {cleanup}")]
    RollbackFailed { cause: String, cleanup: String },
}

/// `<stderr> (`<command>` exited with <code>)`, or only the parenthesised part when the
/// command printed no error.
fn command_failed(command: &str, code: Option<i32>, stderr: &str) -> String {
    let ran = match code {
        Some(code) => format!("`{command}` exited with {code}"),
        None => format!("`{command}` was stopped by a signal"),
    };
    match stderr.trim() {
        "" => ran,
        why => format!("{why} ({ran})"),
    }
}

impl Error {
    pub fn missing(what: &'static str, name: impl Into<String>) -> Self {
        Self::Missing {
            what,
            name: name.into(),
        }
    }

    /// A stable machine name for the log field `error_kind`.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Spawn { .. } => "spawn",
            Self::Command { .. } => "command",
            Self::Io { .. } => "io",
            Self::InvalidName(_) => "invalid_name",
            Self::InvalidInput(_) => "invalid_input",
            Self::NotFound(_) => "not_found",
            Self::Missing { .. } => "missing",
            Self::Conflict(_) => "conflict",
            Self::TooLarge(_) => "too_large",
            Self::Database(_) => "database",
            Self::Unavailable(_) => "unavailable",
            Self::Corrupt { .. } => "corrupt",
            Self::RollbackFailed { .. } => "rollback_failed",
        }
    }

    pub fn io(context: impl Into<String>, source: std::io::Error) -> Self {
        Self::Io {
            context: context.into(),
            source,
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    fn failed(command: &str, code: Option<i32>, stderr: &str) -> String {
        Error::Command {
            command: command.into(),
            code,
            stderr: stderr.into(),
        }
        .to_string()
    }

    #[test]
    fn every_variant_has_its_own_kind() {
        let io = || std::io::Error::other("x");
        let all = [
            (
                Error::Spawn {
                    program: "p".into(),
                    source: io(),
                },
                "spawn",
            ),
            (
                Error::Command {
                    command: "c".into(),
                    code: Some(1),
                    stderr: String::new(),
                },
                "command",
            ),
            (Error::io("ctx", io()), "io"),
            (Error::InvalidName("n".into()), "invalid_name"),
            (Error::InvalidInput("i".into()), "invalid_input"),
            (Error::NotFound("a".into()), "not_found"),
            (Error::missing("task", "t"), "missing"),
            (Error::Conflict("c".into()), "conflict"),
            (Error::TooLarge("t".into()), "too_large"),
            (Error::Database("d".into()), "database"),
            (Error::Unavailable("u".into()), "unavailable"),
            (
                Error::Corrupt {
                    path: "p".into(),
                    message: "m".into(),
                },
                "corrupt",
            ),
            (
                Error::RollbackFailed {
                    cause: "a".into(),
                    cleanup: "b".into(),
                },
                "rollback_failed",
            ),
        ];
        let mut seen = std::collections::HashSet::new();
        for (error, kind) in all {
            assert_eq!(error.kind(), kind);
            assert!(seen.insert(kind), "duplicate kind {kind}");
        }
    }

    #[test]
    fn a_failed_command_says_why_before_what_it_ran() {
        let long = format!("tmux new-session {}", "x".repeat(2_000));
        let text = failed(&long, Some(1), "File name too long");
        assert!(
            text.starts_with("File name too long (`tmux new-session x"),
            "{text}"
        );
        assert!(text.ends_with("x` exited with 1)"), "{text}");
        assert_eq!(
            failed("git x", None, "boom"),
            "boom (`git x` was stopped by a signal)"
        );
        assert_eq!(failed("git x", Some(2), ""), "`git x` exited with 2");
    }
}
