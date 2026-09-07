use super::*;

// --- the IO edge --------------------------------------------------------

#[test]
fn a_missing_file_is_its_own_outcome_not_an_error_and_not_empty() {
    let outcome = load_config(std::path::Path::new("/nonexistent/pns-config-test.toml"));
    assert_eq!(outcome, Ok(LoadOutcome::Missing));
}

#[test]
fn a_present_file_loads_through_the_parser() {
    let path = std::env::temp_dir().join(format!("pns-config-test-{}", std::process::id()));
    std::fs::write(&path, "[plugins.hue]\nenabled = true\n").unwrap();
    let outcome = load_config(&path);
    std::fs::remove_file(&path).ok();
    match outcome {
        Ok(LoadOutcome::Loaded(config)) => assert!(config.plugins["hue"].enabled),
        other => panic!("expected Loaded, got {other:?}"),
    }
}

#[test]
fn a_dangling_config_symlink_is_an_error_never_missing() {
    // chezmoi deploys configs as symlinks: a broken link is a PRESENT
    // entry whose target is wrong, and reading it as "unconfigured"
    // would silently disable everything. Only a truly absent entry is
    // Missing.
    let link = std::env::temp_dir().join(format!("pns-config-dangling-{}", std::process::id()));
    let _ = std::fs::remove_file(&link);
    std::os::unix::fs::symlink("pns-absent-target", &link).unwrap();
    let outcome = load_config(&link);
    std::fs::remove_file(&link).ok();
    match outcome {
        Err(ConfigError::Unreadable(message)) => {
            assert!(!message.is_empty(), "the path and cause are named")
        }
        other => panic!("expected Unreadable, got {other:?}"),
    }
}

#[test]
fn an_unreadable_path_is_an_error_never_a_silent_unconfigured() {
    // A directory at the config path is the deterministic unreadable
    // case: it exists, so reporting Missing here would make a broken
    // path read as "unconfigured" and silently disable everything.
    let outcome = load_config(std::env::temp_dir().as_path());
    match outcome {
        Err(ConfigError::Unreadable(message)) => {
            assert!(!message.is_empty(), "the path and cause are named")
        }
        other => panic!("expected Unreadable, got {other:?}"),
    }
}
