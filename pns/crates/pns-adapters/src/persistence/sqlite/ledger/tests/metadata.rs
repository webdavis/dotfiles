use super::*;
use std::os::unix::fs::PermissionsExt;

// Only the version-4 ledger tables are read in this migration scenario.
// Their original creation function deliberately remains the version-2 shape.
fn schema_four_ledger(path: &std::path::Path) {
    std::fs::create_dir(path).unwrap();
    let database = path.join("pns.db");
    let mut connection = rusqlite::Connection::open(&database).unwrap();
    std::fs::set_permissions(&database, std::fs::Permissions::from_mode(0o600)).unwrap();
    let transaction = connection.transaction().unwrap();
    schema::create(&transaction).unwrap();
    transaction.execute_batch(
        "INSERT INTO ledger_events(producer,request_id,agent,state,project,branch,detail,title,message,preview,pane)
         VALUES ('osquery','legacy-id','agent','done','','','legacy detail','','','','');"
    ).unwrap();
    transaction.pragma_update(None, "user_version", 4).unwrap();
    transaction.commit().unwrap();
}

#[test]
fn schema_four_rows_stay_without_metadata_while_original_request_metadata_survives_and_controls_duplicates()
 {
    let path = state();
    schema_four_ledger(&path);
    let store = SqliteStore::new(path.clone());
    let old_identity = SubmissionIdentity {
        producer: "osquery".into(),
        request_id: "legacy-id".into(),
    };
    let prior = store.inspect(&old_identity);
    assert!(
        prior.is_ok(),
        "schema-four ledger must migrate before reading"
    );
    let old = prior.unwrap().unwrap();
    assert_eq!(
        old.submission.producer_request, None,
        "legacy metadata is unknown"
    );
    assert_eq!(old.submission.event.detail, "legacy detail");

    let encoded = concat!(
        "{\"schema\":\"pns.request/1\",\"request_id\":\"original-id\",\"producer\":\"osquery\",",
        "\"session\":{\"id\":\"session-1\",\"turn\":7},\"event\":\"source-event\",\"signal\":{\"kind\":\"needs_attention\"},",
        "\"occurred_at\":100,\"elapsed_secs\":40,\"detail\":\"private fixture\",",
        "\"context\":{\"project\":\"project\",\"branch\":\"branch\",\"pane\":\"pane\"},",
        "\"scope\":\"automatic\",\"route\":\"urgent\",\"interaction\":{\"kind\":\"none\"},",
        "\"extensions\":{\"nested\":{\"value\":\"retained\"}}}"
    );
    let mut input = submission();
    input.producer_request = Some(encoded.into());
    created(&store, &input);
    let reopened = SqliteStore::new(path.clone());
    let retained = reopened.inspect(&input.identity).unwrap().unwrap();
    assert_eq!(
        retained.submission.producer_request.as_deref(),
        Some(encoded),
        "original metadata must survive reopening"
    );
    assert_eq!(retained.submission, input);
    let duplicate = reopened.prepare(&input, lease(11, 21)).unwrap();
    assert!(
        matches!(duplicate, PreparedSubmission::Existing(ref record) if record.submission == input),
        "identical metadata must return the existing submission"
    );

    input.producer_request = Some(encoded.replace("retained", "changed"));
    assert_eq!(
        reopened.prepare(&input, lease(11, 21)).unwrap_err(),
        LedgerFailure::ConflictingSubmission,
        "changed metadata must conflict even when rendering is identical"
    );
    input.producer_request = None;
    assert_eq!(
        reopened.prepare(&input, lease(11, 21)).unwrap_err(),
        LedgerFailure::ConflictingSubmission,
        "missing metadata must not replace a retained request"
    );
    let mut legacy = old.submission;
    legacy.producer_request = Some(encoded.replace("original-id", "legacy-id"));
    assert_eq!(
        reopened.prepare(&legacy, lease(11, 21)).unwrap_err(),
        LedgerFailure::ConflictingSubmission,
        "legacy metadata must never be guessed or backfilled"
    );

    let claimed = reopened
        .claim_retry(lease(21, 31), Default::default())
        .unwrap()
        .unwrap();
    assert_eq!(claimed.identity, input.identity);
    assert_eq!(
        claimed.event, input.event,
        "retry uses the retained rendering"
    );
    assert_eq!(
        reopened
            .inspect(&input.identity)
            .unwrap()
            .unwrap()
            .submission
            .producer_request
            .as_deref(),
        Some(encoded),
        "claiming a retry must leave original metadata intact"
    );
    assert_eq!(
        reopened
            .inspect(&old_identity)
            .unwrap()
            .unwrap()
            .submission
            .producer_request,
        None
    );
    assert_eq!(
        reopened
            .connect()
            .unwrap()
            .pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
            .unwrap(),
        crate::persistence::sqlite::migrations::VERSION
    );
}
