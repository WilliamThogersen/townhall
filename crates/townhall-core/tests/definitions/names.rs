use townhall_core::cmd::validate_name;
use townhall_core::definitions::{step_agent_name, MAX_STEP_ID_LEN, SHORT_RUN_ID_LEN};
use townhall_core::ids::new_id;
use townhall_core::Error;

#[test]
fn keeps_eight_characters_of_the_run_id() {
    assert_eq!(SHORT_RUN_ID_LEN, 8);
}

#[test]
fn a_random_uuid_gives_its_first_characters() {
    let name = step_agent_name("build", "3f2a9c1e-7b1d-4c6e-9a51-0c2d4e6f8a1b").unwrap();
    assert_eq!(name, "build-3f2a9c1e");
    assert!(validate_name(&name).is_ok());
}

#[test]
fn a_v7_uuid_gives_its_random_tail() {
    assert_eq!(
        step_agent_name("build", "017F22E2-79B0-7CC3-98C4-DC0C0C07398F").unwrap(),
        "build-0c07398f"
    );
}

#[test]
fn v6_uuids_with_one_node_still_get_different_names() {
    let first = step_agent_name("build", "1EC9414C-232A-6B00-B3C8-9F6BDECED846").unwrap();
    let second = step_agent_name("build", "1EC9414D-0A1B-6C00-B3C8-9F6BDECED846").unwrap();
    assert_eq!(first, "build-1ec9414c");
    assert_ne!(first, second);
}

#[test]
fn two_time_ordered_runs_in_one_millisecond_get_different_names() {
    let first = step_agent_name("build", "017f22e2-79b0-7cc3-98c4-dc0c0c07398f").unwrap();
    let second = step_agent_name("build", "017f22e2-79b0-7aaa-8bbb-ccccdddd1234").unwrap();
    assert_ne!(first, second);
}

#[test]
fn a_townhall_id_gives_its_first_characters() {
    let id = new_id();
    let name = step_agent_name("test", &id).unwrap();
    assert_eq!(name, format!("test-{}", &id[..8]));
    assert!(validate_name(&name).is_ok());
}

#[test]
fn refuses_a_run_id_too_short_to_tell_runs_apart() {
    assert!(matches!(
        step_agent_name("build", ""),
        Err(Error::InvalidInput(_))
    ));
    assert!(matches!(
        step_agent_name("build", "3f2a9c1"),
        Err(Error::InvalidInput(_))
    ));
}

#[test]
fn refuses_names_that_would_not_pass_validate_name() {
    assert!(matches!(
        step_agent_name("build", "3f/a9c1e2"),
        Err(Error::InvalidName(_))
    ));
    assert!(matches!(
        step_agent_name("", "3f2a9c1e"),
        Err(Error::InvalidName(_))
    ));
    assert!(matches!(
        step_agent_name("my step", "3f2a9c1e"),
        Err(Error::InvalidName(_))
    ));
}

#[test]
fn a_step_id_of_up_to_55_characters_fits_an_agent_name() {
    assert_eq!(MAX_STEP_ID_LEN, 55);
    let name = step_agent_name(&"s".repeat(55), "3f2a9c1e").unwrap();
    assert_eq!(name.len(), 64);
    assert!(matches!(
        step_agent_name(&"s".repeat(56), "3f2a9c1e"),
        Err(Error::InvalidInput(_))
    ));
}
