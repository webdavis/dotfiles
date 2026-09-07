use super::*;
use crate::config::probes::{parsed, refusal};

#[test]
fn an_empty_config_runs_nothing_and_posts_nothing() {
    let config = parsed("");
    assert_eq!(config.lanes, Lanes::default());
    assert_eq!(config.records, None);
    assert_eq!(config.alerts, None);
}

#[test]
fn an_unknown_top_level_key_is_refused_and_the_file_lists_what_it_serves() {
    let detail = refusal("[lane.herdr]\n");
    assert!(detail.contains("unknown top-level key `lane`"), "{detail}");
    assert!(
        detail.contains("alerts, lanes, records, schedule"),
        "{detail}"
    );
}

#[test]
fn a_malformed_file_is_a_loud_error_and_never_an_empty_config() {
    let detail = refusal("[lanes\n");
    assert!(!detail.is_empty());
    assert!(matches!(
        parse_config("[lanes\n"),
        Err(ConfigError::Malformed(_))
    ));
}

#[test]
fn a_records_block_posts_to_the_unattended_upgrades_route_when_it_names_no_url() {
    let config = parsed("[records]\nkey = \"secret\"\n");
    assert_eq!(
        config.records,
        Some(Records {
            url: DEFAULT_RECORD_URL.to_string(),
            key: "secret".to_string(),
        })
    );
}

#[test]
fn a_records_block_without_a_key_is_refused_because_it_could_never_post() {
    let detail = refusal("[records]\nurl = \"http://example/x\"\n");
    assert!(detail.contains("`records` has no `key`"), "{detail}");
}

#[test]
fn an_alerts_block_finds_the_engine_on_path_when_it_names_no_binary() {
    assert_eq!(
        parsed("[alerts]\n").alerts,
        Some(Alerts {
            binary: DEFAULT_ALERT_BINARY.to_string()
        })
    );
}

#[test]
fn a_path_with_nothing_at_it_is_missing_rather_than_an_error() {
    assert_eq!(
        load_config(Path::new("/nonexistent/uu-config-test.toml")),
        Ok(LoadOutcome::Missing)
    );
}

#[test]
fn a_dangling_config_symlink_is_unreadable_rather_than_missing() {
    // chezmoi deploys configs as symlinks, and a broken link reads
    // NotFound exactly like an absent path. The two are opposite states:
    // an absent config is an unconfigured machine, a broken link is a
    // CONFIGURED machine whose file stopped resolving, and reading it as
    // "unconfigured" turns every lane off without a word.
    let link = std::env::temp_dir().join(format!("uu-config-dangling-{}", std::process::id()));
    let _ = std::fs::remove_file(&link);
    std::os::unix::fs::symlink("uu-absent-target", &link).expect("the link");
    let outcome = load_config(&link);
    std::fs::remove_file(&link).ok();
    assert!(
        matches!(outcome, Err(ConfigError::Unreadable(_))),
        "{outcome:?}"
    );
}

#[test]
fn the_config_lives_under_the_xdg_config_directory() {
    assert_eq!(
        config_path("/home/x"),
        std::path::PathBuf::from("/home/x/.config/uu/config.toml")
    );
}
