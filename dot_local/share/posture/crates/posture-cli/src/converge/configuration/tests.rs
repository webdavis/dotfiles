use super::*;
use std::{collections::BTreeMap, time::Duration};
fn read(values: &[(&str, &str)]) -> Result<Configuration, Vec<ConfigurationRefusal>> {
    let mut environment: BTreeMap<_, _> = [
        ("HOME", OsString::from("/fixture/home")),
        ("PATH", OsString::from("/fixture/bin")),
    ]
    .into();
    for (name, value) in values {
        environment.insert(name, OsString::from(value));
    }
    Configuration::read(|name| environment.get(name).cloned())
}

#[test]
fn test_a_seam_variable_set_without_the_test_seam_is_refused_so_it_is_not_a_production_knob() {
    for name in [
        "OSQUERY_CONVERGE_DESIRED_DIR",
        "OSQUERY_CONVERGE_TARGET_DIR",
        "OSQUERY_CONVERGE_SUDO",
        "OSQUERY_CONVERGE_OSQUERYCTL",
    ] {
        for value in ["", "/fixture/override"] {
            assert_eq!(
                read(&[(name, value)]).unwrap_err(),
                [ConfigurationRefusal::UnexpectedOverride(name)]
            );
        }
    }
}

#[test]
fn every_present_privileged_override_is_reported_in_the_legacy_order() {
    assert_eq!(
        read(&[
            ("OSQUERY_CONVERGE_SUDO", ""),
            ("OSQUERY_CONVERGE_TARGET_DIR", ""),
            ("OSQUERY_CONVERGE_DESIRED_DIR", ""),
            ("OSQUERY_CONVERGE_OSQUERYCTL", "")
        ])
        .unwrap_err(),
        [
            ConfigurationRefusal::UnexpectedOverride("OSQUERY_CONVERGE_DESIRED_DIR"),
            ConfigurationRefusal::UnexpectedOverride("OSQUERY_CONVERGE_TARGET_DIR"),
            ConfigurationRefusal::UnexpectedOverride("OSQUERY_CONVERGE_SUDO"),
            ConfigurationRefusal::UnexpectedOverride("OSQUERY_CONVERGE_OSQUERYCTL")
        ]
    );
}

#[test]
fn test_the_test_seam_without_a_target_directory_is_refused_never_defaulted_to_var_osquery() {
    for value in [None, Some("")] {
        let mut values = vec![
            ("OSQUERY_CONVERGE_TEST_SEAM", "1"),
            ("OSQUERY_CONVERGE_SUDO", "/fixture/sudo"),
        ];
        if let Some(value) = value {
            values.push(("OSQUERY_CONVERGE_TARGET_DIR", value));
        }
        assert_eq!(
            read(&values).unwrap_err(),
            [ConfigurationRefusal::MissingSandboxPath(
                "OSQUERY_CONVERGE_TARGET_DIR"
            )]
        );
    }
}

#[test]
fn test_the_test_seam_without_a_sudo_is_refused_too_so_root_is_never_the_real_one() {
    assert_eq!(
        read(&[
            ("OSQUERY_CONVERGE_TEST_SEAM", "1"),
            ("OSQUERY_CONVERGE_TARGET_DIR", "/fixture/target")
        ])
        .unwrap_err(),
        [ConfigurationRefusal::MissingSandboxPath(
            "OSQUERY_CONVERGE_SUDO"
        )]
    );
}

#[test]
fn a_complete_test_seam_keeps_all_explicit_paths_and_bound_values() {
    let config = read(&[
        ("OSQUERY_CONVERGE_TEST_SEAM", "1"),
        ("OSQUERY_CONVERGE_TARGET_DIR", "/fixture/target"),
        ("OSQUERY_CONVERGE_SUDO", "/fixture/sudo"),
        ("OSQUERY_CONVERGE_DESIRED_DIR", "/fixture/desired"),
        ("OSQUERY_CONVERGE_OSQUERYCTL", "/fixture/ctl"),
        ("OSQUERY_CONVERGE_RESTART_DEADLINE", "12"),
        ("OSQUERY_CONVERGE_SETTLE_SECONDS", "3"),
    ])
    .unwrap();
    assert_eq!(config.target, PathBuf::from("/fixture/target"));
    assert_eq!(config.sudo, PathBuf::from("/fixture/sudo"));
    assert_eq!(config.desired, PathBuf::from("/fixture/desired"));
    assert_eq!(config.osqueryctl, Some(PathBuf::from("/fixture/ctl")));
    assert_eq!(config.bounds.deadline(), Duration::from_secs(12));
    assert_eq!(config.bounds.settle(), Duration::from_secs(3));
}

#[test]
fn the_log_path_and_wait_bounds_are_unprivileged_overrides() {
    let config = read(&[
        ("OSQUERY_CONVERGE_LOG_DIR", "/fixture/log"),
        ("OSQUERY_CONVERGE_RESTART_DEADLINE", "2"),
        ("OSQUERY_CONVERGE_SETTLE_SECONDS", "1"),
    ])
    .unwrap();
    assert_eq!(config.log_directory, PathBuf::from("/fixture/log"));
    assert_eq!(config.bounds.deadline(), Duration::from_secs(2));
    assert_eq!(config.bounds.settle(), Duration::from_secs(1));
}

#[test]
fn ordinary_configuration_uses_the_existing_deployed_paths_and_command_search() {
    let config = read(&[]).unwrap();
    assert_eq!(
        config.desired,
        PathBuf::from("/fixture/home/.local/libexec/osquery/osquery-converge/desired")
    );
    assert_eq!(config.target, PathBuf::from("/var/osquery"));
    assert_eq!(config.sudo, PathBuf::from("/usr/bin/sudo"));
    assert_eq!(config.osqueryctl, None);
    assert_eq!(config.search_path, OsString::from("/fixture/bin"));
    assert_eq!(
        config.log_directory,
        PathBuf::from("/fixture/home/.local/log/osquery")
    );
    assert_eq!(config.bounds.deadline(), Duration::from_secs(30));
    assert_eq!(config.bounds.settle(), Duration::from_secs(5));
}
