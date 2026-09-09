use super::*;
use crate::state_fixtures::scratch;

#[test]
fn setup_refuses_a_directory_before_questions_without_suggesting_force() {
    let path = scratch("setup-directory").join("config.toml");
    std::fs::create_dir(&path).unwrap();
    for force in [false, true] {
        let refusal = check_config_path(&path, force).expect_err("directories cannot be replaced");
        assert!(refusal.contains("is a directory"), "{refusal}");
        assert!(!refusal.contains("pass --force"), "{refusal}");
        assert!(path.is_dir());
    }
}
