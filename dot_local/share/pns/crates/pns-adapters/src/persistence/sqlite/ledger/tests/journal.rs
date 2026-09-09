use super::*;
use pns_application::Journal;
use pns_domain::EventArgs;

fn note(detail: &str) -> EventArgs {
    EventArgs {
        detail: detail.into(),
        ..EventArgs::default()
    }
}

#[test]
fn only_an_acknowledged_decorative_leg_clears_its_original_keyed_miss() {
    let store = SqliteStore::new(state());
    let _open = store.connect().unwrap();
    let input = submission();
    let claims = created(&store, &input);
    let other = SubmissionIdentity {
        producer: "other".into(),
        ..input.identity.clone()
    };
    for (detail, identity) in [
        ("original", Some(&input.identity)),
        ("other", Some(&other)),
        ("legacy", None),
    ] {
        store
            .record_journal(&note(detail), Some(11), identity)
            .unwrap();
    }
    store
        .record(
            &claims[0].claim,
            &reported(&acknowledged()),
            12,
            Default::default(),
        )
        .unwrap();
    assert!(Journal::read(&store).unwrap().unwrap().contains("original"));
    store
        .record(
            &claims[1].claim,
            &reported(&retry(20)),
            12,
            Default::default(),
        )
        .unwrap();
    assert!(Journal::read(&store).unwrap().unwrap().contains("original"));
    let retry = store
        .claim_retry(lease(20, 30), Default::default())
        .unwrap()
        .unwrap();
    store
        .record(
            &retry.claim,
            &reported(&acknowledged()),
            21,
            Default::default(),
        )
        .unwrap();
    let pending = Journal::read(&store).unwrap().unwrap();
    assert!(
        !pending.contains("original"),
        "confirmed original delivery must not raise a later summary"
    );
    assert!(pending.contains("other") && pending.contains("legacy"));
}

#[test]
fn a_completed_original_cannot_be_rejournaled_and_duplicate_pending_identity_is_not_appended() {
    let store = SqliteStore::new(state());
    let _open = store.connect().unwrap();
    let input = submission();
    let claims = created(&store, &input);
    store
        .record_journal(&note("first"), Some(11), Some(&input.identity))
        .unwrap();
    store
        .record_journal(&note("duplicate"), Some(12), Some(&input.identity))
        .unwrap();
    let pending = crate::journal_codec::entries(&Journal::read(&store).unwrap().unwrap());
    assert_eq!(
        pending.len(),
        1,
        "one original submission has one missed record"
    );
    assert_eq!(pending[0].detail, "first");
    store
        .record(
            &claims[1].claim,
            &reported(&acknowledged()),
            13,
            Default::default(),
        )
        .unwrap();
    store
        .record_journal(
            &note("late completion tail"),
            Some(14),
            Some(&input.identity),
        )
        .unwrap();
    assert_eq!(Journal::read(&store).unwrap(), None);
}

#[test]
fn refusing_keyed_miss_removal_rolls_back_completion_and_preserves_the_claim() {
    let store = SqliteStore::new(state());
    let connection = store.connect().unwrap();
    let input = submission();
    let claims = created(&store, &input);
    store
        .record_journal(&note("original"), Some(11), Some(&input.identity))
        .unwrap();
    connection.execute_batch("CREATE TRIGGER retain_miss BEFORE DELETE ON journal BEGIN SELECT RAISE(ABORT, 'owned refusal'); END;").unwrap();
    assert!(matches!(
        store.record(
            &claims[1].claim,
            &reported(&acknowledged()),
            12,
            Default::default()
        ),
        Err(LedgerFailure::Unavailable(_))
    ));
    assert!(matches!(
        store.inspect(&input.identity).unwrap().unwrap().attempts[1].completion,
        LedgerCompletion::Retry {
            outcome: UnconfirmedDelivery::Unknown,
            ..
        }
    ));
    assert!(Journal::read(&store).unwrap().unwrap().contains("original"));
}

#[test]
fn mixed_legacy_and_keyed_journal_appends_preserve_bytes_and_identity_across_pruning() {
    let sql_path = state();
    let file_path = state();
    std::fs::create_dir_all(&sql_path).unwrap();
    std::fs::create_dir_all(&file_path).unwrap();
    let legacy = (0..24)
        .map(|i| format!("legacy {i}\r\n"))
        .collect::<String>();
    for path in [&sql_path, &file_path] {
        std::fs::write(path.join(crate::MISSED_NOTIFICATIONS), &legacy).unwrap();
    }
    let store = SqliteStore::for_records(sql_path);
    let file = crate::FileRecords::new(file_path);
    let _open = store.connect().unwrap();
    let identity = submission().identity;
    for (detail, id) in [("keyed", Some(&identity)), ("later", None)] {
        Journal::journal(&store, &note(detail), Some(1), id);
        Journal::journal(&file, &note(detail), Some(1), None);
        assert_eq!(
            Journal::read(&store).unwrap(),
            Journal::read(&file).unwrap()
        );
    }
    let before = Journal::read(&store).unwrap();
    Journal::journal(&store, &note("duplicate"), Some(2), Some(&identity));
    assert_eq!(
        Journal::read(&store).unwrap(),
        before,
        "an unrelated append must retain the keyed row identity"
    );
}
