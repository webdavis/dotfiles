use super::{TURN_SUMMARIZER_CEILING, summarizer_home, turn_deadline};
use std::os::unix::fs::PermissionsExt;

#[test]
fn summarizer_creates_owner_only_home_and_config_with_only_the_owned_auth_link() {
    let root = crate::state_fixtures::scratch("summarizer-private-modes");
    let user = root.join("fixture-user");
    std::fs::create_dir_all(user.join(".codex")).unwrap();
    std::fs::write(user.join(".codex/auth.json"), "owned credential fixture").unwrap();
    let home = summarizer_home(user.to_str().unwrap(), None).unwrap();
    assert_eq!(home, user.join(".config/pns/codex-home"));
    assert_eq!(
        std::fs::metadata(&home).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        std::fs::metadata(home.join("config.toml"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert_eq!(
        std::fs::read_to_string(home.join("config.toml")).unwrap(),
        "model = \"gpt-5.5\"\nmodel_reasoning_effort = \"low\"\n"
    );
    assert_eq!(
        std::fs::read_link(home.join("auth.json")).unwrap(),
        user.join(".codex/auth.json")
    );
    assert_eq!(
        std::fs::read_to_string(user.join(".codex/auth.json")).unwrap(),
        "owned credential fixture"
    );
    let mut names: Vec<_> = std::fs::read_dir(&home)
        .unwrap()
        .map(|f| f.unwrap().file_name())
        .collect();
    names.sort();
    assert_eq!(names, ["auth.json", "config.toml"]);
}

#[test]
fn summarizer_preserves_an_existing_config_and_uses_the_explicit_private_home() {
    let root = crate::state_fixtures::scratch("summarizer-existing-config");
    let home = root.join("chosen");
    std::fs::create_dir(&home).unwrap();
    std::fs::write(home.join("config.toml"), "existing fixture configuration").unwrap();
    assert_eq!(
        summarizer_home(root.to_str().unwrap(), home.to_str()),
        Some(home.clone())
    );
    assert_eq!(
        std::fs::read_to_string(home.join("config.toml")).unwrap(),
        "existing fixture configuration"
    );
    assert_eq!(
        std::fs::read_link(home.join("auth.json")).unwrap(),
        root.join(".codex/auth.json")
    );
}

#[test]
fn the_turn_summarizer_takes_the_configured_bound_up_to_its_own_ceiling() {
    // A Stop hook is blocked on this call, so the recap's generous budget
    // reaches it only as far as the ceiling; a shorter bound is taken whole.
    use std::time::Duration;
    assert_eq!(
        turn_deadline(Duration::from_secs(240)),
        TURN_SUMMARIZER_CEILING
    );
    assert_eq!(
        turn_deadline(Duration::from_millis(300)),
        Duration::from_millis(300)
    );
}
