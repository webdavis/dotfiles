use super::*;
use std::os::unix::ffi::OsStringExt;

#[test]
fn home_names_the_spool_the_alerter_writes_and_the_engine_that_delivers() {
    let home = OsString::from_vec(b"/private/a home \xff".to_vec());
    let mut reads = Vec::new();
    let config = Configuration::read(|key| {
        reads.push(key.to_owned());
        match key {
            "HOME" => Some(home.clone()),
            "OSQUERY_DIGEST_STORE" => None,
            "DIGEST_MAX_GROUPS"
            | "DIGEST_MAX_BULLETS_PER_GROUP"
            | "DIGEST_MAX_BODY_CHARS"
            | "DIGEST_MAX_FIELD_CHARS" => None,
            other => panic!("unexpected environment read: {other}"),
        }
    })
    .unwrap();
    assert_eq!(
        reads,
        [
            "HOME",
            "OSQUERY_DIGEST_STORE",
            "DIGEST_MAX_GROUPS",
            "DIGEST_MAX_BULLETS_PER_GROUP",
            "DIGEST_MAX_BODY_CHARS",
            "DIGEST_MAX_FIELD_CHARS"
        ]
    );
    let mut store = home.clone();
    store.push(DEFAULT_STORE);
    assert_eq!(config.store, PathBuf::from(store));
    let mut engine = home;
    engine.push("/.cargo/bin/pns");
    assert_eq!(config.pns, PathBuf::from(engine));
    assert_eq!(config.alarm, PathBuf::from("/usr/bin/osascript"));
    assert!(Configuration::read(|_| None).is_none());
}

#[test]
fn an_override_names_the_spool_but_an_empty_one_does_not() {
    // An empty override would resolve against the process's working directory,
    // and the run would claim and rotate files wherever launchd happened to
    // start it.
    let read = |value: &str| {
        Configuration::read(|key| match key {
            "HOME" => Some("/home".into()),
            _ => Some(value.into()),
        })
        .unwrap()
        .store
    };
    assert_eq!(
        read("/elsewhere/spool.ndjson"),
        PathBuf::from("/elsewhere/spool.ndjson")
    );
    assert_eq!(read(""), PathBuf::from(format!("/home{DEFAULT_STORE}")));
}

#[test]
fn absent_or_malformed_scalars_keep_each_original_default() {
    for value in [
        None,
        Some(""),
        Some("abc"),
        Some("-1"),
        Some("+1"),
        Some(" 1"),
        Some("1.5"),
    ] {
        let config = Configuration::read(|key| match key {
            "HOME" => Some("/private/test-home".into()),
            "OSQUERY_DIGEST_STORE" => None,
            _ => value.map(OsString::from),
        })
        .unwrap();
        assert_eq!(
            config.limits,
            DigestLimits {
                groups: 12,
                bullets_per_group: 10,
                body_chars: 1800,
                field_chars: 240,
            }
        );
    }
}
