use super::{SqliteStore, state};
#[test]
fn presence_history_retains_five_exact_codec_lines_and_the_latest_narrowing() {
    let store = SqliteStore::new(state());
    let _open = store.connect().unwrap();
    let mut expected = Vec::new();
    for at in 0..6 {
        let entry = pns_domain::PresenceDecision {
            at: Some(at),
            presence: "room \"A\"".into(),
            desk_idle_secs: None,
            home: "home".into(),
            room: Some(format!("Room {at}\nannex")),
            reason: String::new(),
        };
        store.record_presence(&entry).unwrap();
        expected.push(crate::presence_journal::entry(&entry));
        if at == 4 {
            assert_eq!(
                store.presence_history().unwrap().unwrap(),
                format!("{}\n", expected.join("\n"))
            );
        }
    }
    let held = store.presence_history().unwrap().unwrap();
    assert_eq!(held, format!("{}\n", expected[1..].join("\n")));
    assert_eq!(
        crate::presence_journal::last(&held)
            .unwrap()
            .room
            .as_deref(),
        Some("Room 5\nannex")
    );
}
#[test]
fn policy_settings_history_retains_twenty_receipts_with_unknown_clock_and_empty_path_wording() {
    let store = SqliteStore::new(state());
    let _open = store.connect().unwrap();
    for n in 0..20 {
        store
            .record_policy_settings_change(&format!("s{n}"), "", None)
            .unwrap();
    }
    let before = store.policy_settings_history().unwrap().unwrap();
    assert_eq!(before.lines().count(), 20);
    assert!(before.starts_with("0 session=s0 file=none\n"));
    store
        .record_policy_settings_change("last", "/project/settings.json", Some(u64::MAX))
        .unwrap();
    let after = store.policy_settings_history().unwrap().unwrap();
    assert_eq!(after.lines().count(), 20);
    assert!(after.starts_with("0 session=s1 file=none\n"));
    assert!(after.ends_with("18446744073709551615 session=last file=/project/settings.json\n"));
}
