use super::*;

fn env(pairs: &'static [(&'static str, &'static str)]) -> impl FnMut(&str) -> Option<OsString> {
    move |name: &str| {
        pairs
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| OsString::from(*value))
    }
}

#[test]
fn without_a_home_there_is_nothing_to_resolve_against() {
    assert!(Configuration::read(env(&[])).is_none());
}

#[test]
fn every_path_defaults_under_the_operators_own_home() {
    let config = Configuration::read(env(&[("HOME", "/Users/someone")])).expect("a config");
    assert_eq!(config.home, "/Users/someone");
    assert_eq!(
        config.log,
        PathBuf::from("/Users/someone/.local/log/osquery/osqueryd.results.log")
    );
    assert_eq!(
        config.cursor,
        PathBuf::from("/Users/someone/.local/state/osquery-results-offset")
    );
    assert_eq!(
        config.allowlist,
        PathBuf::from("/Users/someone/.config/osquery/page-launchd-allowlist.txt")
    );
    assert_eq!(
        config.spool,
        PathBuf::from("/Users/someone/.local/state/osquery-digest-spool/digest.ndjson")
    );
    assert_eq!(config.pns, PathBuf::from("/Users/someone/.cargo/bin/pns"));
}

#[test]
fn each_path_can_be_pointed_somewhere_else_for_a_test_run() {
    let config = Configuration::read(env(&[
        ("HOME", "/Users/someone"),
        ("OSQUERY_RESULTS_LOG", "/tmp/log"),
        ("OSQUERY_RESULTS_OFFSET", "/tmp/cursor"),
        ("OSQUERY_LAUNCHD_ALLOWLIST", "/tmp/allowlist"),
        ("OSQUERY_DIGEST_STORE", "/tmp/spool"),
    ]))
    .expect("a config");
    assert_eq!(config.log, PathBuf::from("/tmp/log"));
    assert_eq!(config.cursor, PathBuf::from("/tmp/cursor"));
    assert_eq!(config.allowlist, PathBuf::from("/tmp/allowlist"));
    assert_eq!(config.spool, PathBuf::from("/tmp/spool"));
}

#[test]
fn an_empty_override_is_not_an_override() {
    // It would name the process's working directory. For the cursor that puts
    // the alerter's state wherever launchd happened to start it; for the log it
    // reads a directory as a file on every tick.
    let config = Configuration::read(env(&[
        ("HOME", "/Users/someone"),
        ("OSQUERY_RESULTS_LOG", ""),
        ("OSQUERY_RESULTS_OFFSET", ""),
        ("OSQUERY_LAUNCHD_ALLOWLIST", ""),
        ("OSQUERY_DIGEST_STORE", ""),
    ]))
    .expect("a config");
    assert_eq!(
        config.log,
        PathBuf::from("/Users/someone/.local/log/osquery/osqueryd.results.log")
    );
    assert_eq!(
        config.cursor,
        PathBuf::from("/Users/someone/.local/state/osquery-results-offset")
    );
    assert_eq!(
        config.spool,
        PathBuf::from("/Users/someone/.local/state/osquery-digest-spool/digest.ndjson")
    );
}
