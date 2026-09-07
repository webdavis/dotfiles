use super::*;

// --- path resolution ----------------------------------------------------

#[test]
fn the_config_lives_under_the_homes_dot_config_pns() {
    assert_eq!(
        config_path("/Users/operator"),
        std::path::PathBuf::from("/Users/operator/.config/pns/config.toml")
    );
}
