use super::{SqliteStore, state};
use pns_application::{DecisionRing, Journal};
use pns_domain::{EventArgs, lights::phase::HeldEntry};
use std::fs;
#[test]
fn first_run_import_preserves_raw_history_and_all_scalar_families_without_changing_legacy_bytes() {
    let path = state();
    fs::create_dir(&path).unwrap();
    let files = [
        ("decisions", "9 old decision\r\n10 no final newline"),
        ("missed-notifications", "unrecognized raw journal\r\n"),
        ("quiet-until", "23\n"),
        ("home-staleness", " episode A \n"),
        ("lights-said", "tick refusal\n"),
        ("lights-quiet-said", "mute refusal\n"),
        ("lights-held", "light/one light/two@bad\n"),
        ("lights-quiet", "42 Kitchen\n"),
        ("lights-news", "3 4\n"),
        ("lights-streak", "1 2\n"),
        ("last-present", " 17 \n"),
    ];
    for (name, body) in files {
        fs::write(path.join(name), body).unwrap();
    }
    let store = SqliteStore::new(path.clone());
    assert!(store.import_legacy().unwrap().is_empty());
    assert_eq!(
        DecisionRing::read(&store).unwrap().as_deref(),
        Some(files[0].1)
    );
    assert_eq!(Journal::read(&store).unwrap().as_deref(), Some(files[1].1));
    assert_eq!(store.quiet_expiry().unwrap(), Some(23));
    assert_eq!(store.staleness().unwrap().as_deref(), Some("episode A"));
    assert_eq!(
        store.lights_complaint().unwrap().as_deref(),
        Some("tick refusal\n")
    );
    assert_eq!(
        store.quiet_complaint().unwrap().as_deref(),
        Some("mute refusal\n")
    );
    assert_eq!(
        store.read_held().unwrap(),
        [HeldEntry::bare("light/one"), HeldEntry::bare("light/two")]
    );
    assert_eq!(store.read_muted().unwrap()[0].place, "Kitchen");
    assert_eq!(store.read_news().unwrap().failed_at, Some(4));
    assert_eq!(store.advance_streak(false, 3).unwrap().unwrap().since, 1);
    assert_eq!(store.last_present().unwrap(), Some(17));
    for (name, body) in files {
        assert_eq!(
            fs::read(path.join(name)).unwrap(),
            body.as_bytes(),
            "{name}"
        );
    }
}
#[test]
fn a_completed_import_never_replays_old_files_over_newer_records_or_duplicates_history() {
    let path = state();
    fs::create_dir(&path).unwrap();
    fs::write(path.join("missed-notifications"), "old raw line\n").unwrap();
    fs::write(path.join("quiet-until"), "5\n").unwrap();
    let store = SqliteStore::new(path.clone());
    store.import_legacy().unwrap();
    store
        .record_journal(
            &EventArgs {
                detail: "new event".into(),
                ..EventArgs::default()
            },
            Some(9),
        )
        .unwrap();
    store.set_quiet_expiry(None).unwrap();
    let before = Journal::read(&store).unwrap();
    fs::write(path.join("quiet-until"), "99\n").unwrap();
    store.import_legacy().unwrap();
    assert_eq!(store.quiet_expiry().unwrap(), None);
    assert_eq!(Journal::read(&store).unwrap(), before);
    assert!(before.unwrap().starts_with("old raw line\n"));
}
#[test]
fn unreadable_imports_are_reported_and_remain_unknown_until_the_owning_repository_is_written() {
    let path = state();
    fs::create_dir(&path).unwrap();
    fs::create_dir(path.join("lights-held")).unwrap();
    fs::write(path.join("lights-quiet"), [0xff]).unwrap();
    fs::write(path.join("home-staleness"), "known episode\n").unwrap();
    let mut store = SqliteStore::new(path.clone());
    store.log = path.join("import.log");
    let failures = store.import_legacy().unwrap();
    let logged =
        fs::read_to_string(&store.log).expect("unreadable imports must reach the daemon log");
    assert!(logged.contains("lights-held") && logged.contains("lights-quiet"));
    assert_eq!(
        failures
            .iter()
            .map(|failure| failure.record.as_str())
            .collect::<Vec<_>>(),
        ["lights-held", "lights-quiet"]
    );
    assert_eq!(store.import_failures().unwrap(), failures);
    assert!(
        store.read_held().is_err(),
        "an unreadable held record must not become an ordinary empty house"
    );
    assert!(
        store.read_muted().is_err(),
        "an unreadable mute must remain visible to its fail-dark caller"
    );
    assert_eq!(store.staleness().unwrap().as_deref(), Some("known episode"));
    store.remember_held(&[]).unwrap();
    store.write_muted(&[]).unwrap();
    assert!(store.read_held().unwrap().is_empty());
    assert!(store.read_muted().unwrap().is_empty());
    assert!(store.import_failures().unwrap().is_empty());
    assert!(path.join("lights-held").is_dir());
    assert_eq!(fs::read(path.join("lights-quiet")).unwrap(), [0xff]);
}
#[test]
fn failure_to_commit_import_completion_rolls_back_every_imported_record_and_can_be_retried() {
    let path = state();
    fs::create_dir(&path).unwrap();
    fs::write(path.join("decisions"), "old decision\n").unwrap();
    let store = SqliteStore::new(path);
    let connection = store.connect().unwrap();
    connection.execute_batch("CREATE TRIGGER refuse_import BEFORE INSERT ON legacy_imports BEGIN SELECT RAISE(ABORT, 'fixture refusal'); END;").unwrap();
    assert!(store.import_legacy().is_err());
    assert_eq!(DecisionRing::read(&store).unwrap(), None);
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM legacy_imports", [], |row| row
                .get::<_, u32>(0))
            .unwrap(),
        0
    );
    connection
        .execute_batch("DROP TRIGGER refuse_import")
        .unwrap();
    store.import_legacy().unwrap();
    assert_eq!(
        DecisionRing::read(&store).unwrap().as_deref(),
        Some("old decision\n")
    );
}
#[test]
fn an_empty_imported_ring_stays_distinct_from_an_absent_ring() {
    let path = state();
    fs::create_dir(&path).unwrap();
    fs::write(path.join("decisions"), "").unwrap();
    let store = SqliteStore::new(path);
    store.import_legacy().unwrap();
    assert_eq!(DecisionRing::read(&store).unwrap(), Some(String::new()));
    assert_eq!(Journal::read(&store).unwrap(), None);
}

#[test]
fn unreadable_notification_and_history_imports_remain_diagnostic_until_replaced() {
    let path = state();
    fs::create_dir(&path).unwrap();
    for name in ["quiet-until", "decisions", "home-staleness", "lights-news"] {
        fs::create_dir(path.join(name)).unwrap();
    }
    let store = SqliteStore::new(path);
    store.import_legacy().unwrap();
    assert!(store.quiet_expiry().is_err());
    assert!(DecisionRing::read(&store).is_err());
    assert!(store.staleness().is_err());
    assert!(store.read_news().is_err());
    store.set_quiet_expiry(Some(6)).unwrap();
    store.remember_staleness(None).unwrap();
    assert_eq!(store.quiet_expiry().unwrap(), Some(6));
    assert_eq!(store.staleness().unwrap(), None);
}
#[test]
fn appending_imported_history_matches_legacy_separator_and_pruning_bytes() {
    for initial in ["", "0 first\r\n1 second", "0 first\r\n1 second\r\n"] {
        let prefix = (0..18 - initial.lines().count())
            .map(|n| format!("{n} prior\n"))
            .collect::<String>();
        let initial = format!("{prefix}{initial}");
        let path = state();
        fs::create_dir(&path).unwrap();
        let legacy = path.join("comparison");
        fs::write(&legacy, &initial).unwrap();
        fs::write(path.join("policy-settings-audit"), &initial).unwrap();
        let store = SqliteStore::new(path);
        store.import_legacy().unwrap();
        let _open = store.connect().unwrap();
        assert_eq!(store.policy_settings_history().unwrap().unwrap(), initial);
        for now in 0..3 {
            let line = format!("{now} session=s file=none");
            crate::append_ring_line(&legacy, &line, 20, crate::RING_READ_MAX).unwrap();
            store
                .record_policy_settings_change("s", "", Some(now))
                .unwrap();
            assert_eq!(
                store.policy_settings_history().unwrap().unwrap().as_bytes(),
                fs::read(&legacy).unwrap(),
                "initial {initial:?}, append {now}"
            );
        }
    }
}
#[test]
fn a_live_legacy_return_owner_defers_import_without_marking_any_family_complete() {
    let path = state();
    fs::create_dir(&path).unwrap();
    let hold = path.join(format!(
        "missed-notifications.held.{}.0",
        std::process::id()
    ));
    fs::write(&hold, "held batch\n").unwrap();
    fs::write(path.join("decisions"), "prior decision\n").unwrap();
    let store = SqliteStore::new(path);
    assert!(
        store.import_legacy().is_err(),
        "a live old-binary replay owns this batch"
    );
    let connection = store.connect().unwrap();
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM legacy_imports", [], |row| row
                .get::<_, u32>(0))
            .unwrap(),
        0
    );
    assert_eq!(DecisionRing::read(&store).unwrap(), None);
    assert_eq!(fs::read(hold).unwrap(), b"held batch\n");
}

#[test]
fn malformed_imported_epochs_and_lamp_records_keep_their_original_fail_directions_and_report_the_loss()
 {
    let path = state();
    fs::create_dir(&path).unwrap();
    for (name, body) in [
        ("last-present", "torn"),
        ("quiet-until", "9223372036854775808"),
        ("lights-news", "1 broken"),
        ("lights-streak", "1 broken"),
        ("lights-quiet", "42 "),
    ] {
        fs::write(path.join(name), body).unwrap();
    }
    let store = SqliteStore::new(path);
    let failures = store.import_legacy().unwrap();
    assert_eq!(
        failures
            .iter()
            .map(|failure| failure.record.as_str())
            .collect::<Vec<_>>(),
        [
            "last-present",
            "lights-news",
            "lights-quiet",
            "lights-streak",
            "quiet-until"
        ]
    );
    assert_eq!(
        store.last_present().unwrap(),
        None,
        "an invalid edge cannot become epoch zero"
    );
    assert!(store.quiet_expiry().is_err());
    assert!(store.read_news().is_err());
    assert!(store.read_muted().is_err());
    let claim = store.claim_return(Some(12), true).unwrap().unwrap();
    assert_eq!(claim.since, None);
    assert_eq!(store.last_present().unwrap(), Some(12));
    store
        .record_news(pns_domain::lamps::config::Behaviour::Done, Some(13))
        .unwrap();
    assert_eq!(store.read_news().unwrap().done_at, Some(13));
    assert_eq!(store.advance_streak(true, 14).unwrap().unwrap().since, 14);
}
