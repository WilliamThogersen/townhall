use crate::cmd::validate_name;
use crate::{Error, Result};

/// How many characters of the run id a step agent's name keeps.
pub const SHORT_RUN_ID_LEN: usize = 8;

/// The longest step id, so `<step id>-<short run id>` fits in 64 characters.
pub const MAX_STEP_ID_LEN: usize = 64 - 1 - SHORT_RUN_ID_LEN;

/// The running agent's name for a step: `<step id>-<short run id>`, for example
/// `build-3f2a9c1e`. Always passes `validate_name`.
///
/// The short id comes from the run id's random characters: the tail of a v7
/// UUID, else the start, as with `ids::new_id` or a v4 UUID.
pub fn step_agent_name(step_id: &str, run_id: &str) -> Result<String> {
    let short = short_run_id(run_id)?;
    if step_id.len() > MAX_STEP_ID_LEN {
        return Err(Error::InvalidInput(format!(
            "step id `{step_id}` is longer than {MAX_STEP_ID_LEN} characters"
        )));
    }
    validate_name(step_id)?;
    validate_name(&short)?;
    Ok(format!("{step_id}-{short}"))
}

fn short_run_id(run_id: &str) -> Result<String> {
    let simple = uuid::Uuid::try_parse(run_id)
        .map(|id| (id.get_version_num(), id.simple().to_string()))
        .ok();
    let chars: Vec<char> = match &simple {
        Some((_, hex)) => hex.chars().collect(),
        None => run_id.chars().collect(),
    };
    if chars.len() < SHORT_RUN_ID_LEN {
        return Err(Error::InvalidInput(format!(
            "run id `{run_id}` is too short: it needs {SHORT_RUN_ID_LEN} characters"
        )));
    }
    let random = match simple {
        Some((7, _)) => &chars[chars.len() - SHORT_RUN_ID_LEN..],
        _ => &chars[..SHORT_RUN_ID_LEN],
    };
    Ok(random.iter().collect())
}
