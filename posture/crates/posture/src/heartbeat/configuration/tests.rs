use super::*;
use std::os::unix::ffi::OsStringExt;
#[test]
fn home_constructs_paths_without_reading_legacy_path_overrides() {
    let home = OsString::from_vec(b"/private/a home \xff".to_vec());
    let mut reads = Vec::new();
    let config = Configuration::read(|key| {
        reads.push(key.to_owned());
        match key {
            "HOME" => Some(home.clone()),
            "OSQUERY_CANARY_MAX_AGE" => Some("020".into()),
            other => panic!("unexpected environment read: {other}"),
        }
    })
    .unwrap();
    assert_eq!(reads, ["HOME", "OSQUERY_CANARY_MAX_AGE"]);
    let mut snapshots = home.clone();
    snapshots.push("/.local/log/osquery/osqueryd.snapshots.log");
    assert_eq!(config.snapshots, PathBuf::from(snapshots));
    let mut engine = home;
    engine.push("/.local/libexec/pns/pns");
    assert_eq!(config.pns, PathBuf::from(engine));
    assert_eq!(config.alarm, PathBuf::from("/usr/bin/osascript"));
    assert_eq!(config.maximum_age.seconds(), 16);
    assert_eq!(config.maximum_age.display(), "020");
    assert!(Configuration::read(|_| None).is_none());
}
