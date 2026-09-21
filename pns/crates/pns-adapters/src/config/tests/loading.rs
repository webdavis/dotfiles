use super::*;

// --- path resolution ----------------------------------------------------

#[test]
fn the_config_lives_under_the_homes_dot_config_pns() {
    assert_eq!(
        config_path("/Users/operator"),
        std::path::PathBuf::from("/Users/operator/.config/pns/config.toml")
    );
}

#[test]
fn the_profiles_table_reaches_the_config() {
    let config = parse_config(
        "[profiles.default]\nquiet = false\n\
         [profiles.night]\nquiet = true\nbanner = \"none\"\ndiscord = \"priority\"\n\
         phone = \"priority\"\nlights = \"none\"\n\
         [[profiles.rules]]\nprofile = \"night\"\nhours = \"22:00-06:00\"\n",
    )
    .expect("it loads");
    assert_eq!(config.profiles.len(), 2);
    assert_eq!(config.profile_rules.len(), 1);
    assert_eq!(config.location_poll_secs, 30);
}

#[test]
fn a_config_with_no_profiles_table_has_the_default_profile_and_no_rules() {
    let config = parse_config("").expect("it loads");
    assert!(config.profiles.is_empty(), "no table names no profile");
    assert!(config.profile_rules.is_empty());
    assert_eq!(config.location_poll_secs, 30);
}
