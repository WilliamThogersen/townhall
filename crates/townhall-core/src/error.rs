use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to run `{program}`: {source}")]
    Spawn {
        program: String,
        #[source]
        source: std::io::Error,
    },
    #[error("`{command}` exited with {code:?}: {stderr}")]
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

impl Error {
    pub fn missing(what: &'static str, name: impl Into<String>) -> Self {
        Self::Missing {
            what,
            name: name.into(),
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
