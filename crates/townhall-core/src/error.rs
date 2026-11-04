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
    #[error("{cause}; rollback also failed: {cleanup}")]
    RollbackFailed { cause: String, cleanup: String },
}

pub type Result<T> = std::result::Result<T, Error>;
