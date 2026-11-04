use std::path::Path;

use tokio::process::Command;

use crate::{Error, Result};

/// Run `program args...` (optionally in `cwd`) and return trimmed stdout.
pub async fn run(program: &str, args: &[&str], cwd: Option<&Path>) -> Result<String> {
    let mut command = Command::new(program);
    command.args(args);
    if let Some(dir) = cwd {
        command.current_dir(dir);
    }
    let output = command.output().await.map_err(|source| Error::Spawn {
        program: program.to_string(),
        source,
    })?;
    if !output.status.success() {
        return Err(Error::Command {
            command: format!("{program} {}", args.join(" ")),
            code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Agent names become branch, directory and tmux session names.
pub fn validate_name(name: &str) -> Result<()> {
    let ok = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if ok {
        Ok(())
    } else {
        Err(Error::InvalidName(name.to_string()))
    }
}
