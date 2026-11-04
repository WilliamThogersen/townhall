use townhall_core::cmd::validate_name;
use townhall_core::definitions::step_agent_name;
use townhall_core::Error;

#[test]
fn names_a_step_agent_after_the_step_and_the_short_run_id() {
    let name = step_agent_name("build", "3f2a9c1e-7b1d-4c6e-9a51-0c2d4e6f8a1b").unwrap();
    assert_eq!(name, "build-3f2a");
    assert!(validate_name(&name).is_ok());
}

#[test]
fn two_runs_get_different_names() {
    let first = step_agent_name("build", "3f2a9c1e").unwrap();
    let second = step_agent_name("build", "7c01aa00").unwrap();
    assert_ne!(first, second);
}

#[test]
fn a_short_run_id_is_used_whole() {
    assert_eq!(step_agent_name("test", "ab").unwrap(), "test-ab");
}

#[test]
fn refuses_an_empty_run_id() {
    assert!(matches!(
        step_agent_name("build", ""),
        Err(Error::InvalidInput(_))
    ));
}

#[test]
fn refuses_names_that_would_not_pass_validate_name() {
    assert!(matches!(
        step_agent_name("build", "3f/a"),
        Err(Error::InvalidName(_))
    ));
    assert!(matches!(
        step_agent_name("", "3f2a"),
        Err(Error::InvalidName(_))
    ));
    assert!(matches!(
        step_agent_name("my step", "3f2a"),
        Err(Error::InvalidName(_))
    ));
}
