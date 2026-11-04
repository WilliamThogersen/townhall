use crate::cmd::validate_name;
use crate::{Error, Result};

/// How many characters of the run id a step agent's name keeps.
pub const SHORT_RUN_ID_LEN: usize = 4;

/// The running agent's name for a step: `<step id>-<short run id>`, for example
/// `build-3f2a`. Always passes `validate_name`.
pub fn step_agent_name(step_id: &str, run_id: &str) -> Result<String> {
    let short: String = run_id.chars().take(SHORT_RUN_ID_LEN).collect();
    if short.is_empty() {
        return Err(Error::InvalidInput("run id is empty".into()));
    }
    validate_name(step_id)?;
    validate_name(&short)?;
    Ok(format!("{step_id}-{short}"))
}
