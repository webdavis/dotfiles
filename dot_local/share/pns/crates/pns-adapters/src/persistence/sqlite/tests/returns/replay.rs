use super::*;
use crate::persistence::sqlite::tests::processes::Owned;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::Instant;

#[test]
fn an_abandoned_replay_keeps_its_batch_and_window_separate_from_later_arrivals() {
    adoption(false);
}

#[test]
fn a_queued_replay_retains_original_identity_and_never_retries_its_acknowledged_leg() {
    adoption(true);
}

fn adoption(queued: bool) {
    const KEY: &str = "PNS_RETURN_IDENTITY_CHILD";
    if let Some(path) = std::env::var_os(KEY) {
        let path = std::path::PathBuf::from(path);
        let store = SqliteStore::new(path.clone());
        store.mark_present(10).unwrap();
        store
            .record_journal(&event("original"), Some(11), None)
            .unwrap();
        let claim = store.claim_return(Some(20), true).unwrap().unwrap();
        let identity = claim.replay.unwrap().identity;
        std::fs::write(path.join("original-key"), &identity.request_id).unwrap();
        if std::env::var("PNS_RETURN_QUEUED_CHILD").as_deref() == Ok("yes") {
            queue(&store, identity);
        }
        return;
    }
    let path = state();
    let store = SqliteStore::new(path.clone());
    let connection = store.connect().unwrap();
    let expires = Instant::now() + Duration::from_millis(650);
    let mut child = Owned(Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "persistence::sqlite::tests::returns::replay::an_abandoned_replay_keeps_its_batch_and_window_separate_from_later_arrivals"])
        .env(KEY, &path).env("PNS_RETURN_QUEUED_CHILD", if queued { "yes" } else { "no" }).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
        .process_group(0).spawn().unwrap());
    loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        assert!(
            Instant::now() < expires,
            "owned return fixture did not finish"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    let before: i64 = connection
        .query_row("SELECT id FROM return_claims", [], |r| r.get(0))
        .unwrap();
    store
        .record_journal(&event("later"), Some(21), None)
        .unwrap();
    let claim = store.claim_return(Some(30), true).unwrap().unwrap();
    let after: i64 = connection
        .query_row("SELECT id FROM return_claims", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        after, before,
        "adoption must retain the original batch identity"
    );
    assert_eq!(
        claim.since,
        Some(10),
        "adoption must retain the original window"
    );
    assert_eq!(
        claim
            .waiting
            .iter()
            .map(|e| e.detail.as_str())
            .collect::<Vec<_>>(),
        ["original"]
    );
    assert!(Journal::read(&store).unwrap().unwrap().contains("later"));
    let batch = claim.replay.unwrap();
    assert_eq!(
        batch.identity.request_id,
        std::fs::read_to_string(path.join("original-key")).unwrap()
    );
    assert_eq!(batch.until, Some(20));
    assert_eq!(
        batch.state,
        if queued {
            pns_application::ReplayState::Queued
        } else {
            pns_application::ReplayState::Unsubmitted
        }
    );
    if queued {
        use pns_application::DeliveryLedger;
        let retry = store
            .claim_retry(
                pns_application::LeaseWindow { now: 30, until: 40 },
                Default::default(),
            )
            .unwrap()
            .unwrap();
        assert_eq!(retry.identity, batch.identity);
        assert_eq!(retry.leg.destination, "hermes");
        assert_eq!(retry.leg.route, "original-route");
        assert_eq!(retry.event.detail, "original-summary");
        store
            .record(
                &retry.claim,
                &pns_application::LedgerCompletion::Acknowledged {
                    detail: "accepted".into(),
                },
                31,
            )
            .unwrap();
        assert!(
            store
                .claim_retry(
                    pns_application::LeaseWindow { now: 40, until: 50 },
                    Default::default()
                )
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn an_empty_journal_digest_still_has_one_owned_return_batch() {
    let path = state();
    let store = SqliteStore::new(path.clone());
    let connection = store.connect().unwrap();
    store.mark_present(10).unwrap();
    assert!(store.claim_return(Some(20), true).unwrap().is_some());
    let count: i64 = connection
        .query_row("SELECT count(*) FROM return_claims", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        count, 1,
        "a digest-only card also needs a durable replay identity"
    );
    assert!(
        SqliteStore::new(path)
            .claim_return(Some(20), true)
            .unwrap()
            .is_none(),
        "a concurrent return must not own another card"
    );
}

fn queue(store: &SqliteStore, identity: pns_application::SubmissionIdentity) {
    use pns_application::{
        DeliveryLedger, LeaseWindow, LedgerCompletion, LedgerLeg, LedgerSubmission,
        PreparedSubmission, UnconfirmedDelivery,
    };
    let input = LedgerSubmission {
        producer_request: None,
        identity,
        event: pns_domain::Event {
            detail: "original-summary".into(),
            ..Default::default()
        },
        legs: ["phone", "hermes"]
            .into_iter()
            .map(|name| LedgerLeg {
                destination: name.into(),
                route: "original-route".into(),
                mode: pns_domain::routing::ReportMode::ReportOutcome,
                decorative: name == "phone",
            })
            .collect(),
    };
    let PreparedSubmission::Created { legs, .. } = store
        .prepare(&input, LeaseWindow { now: 20, until: 30 })
        .unwrap()
    else {
        panic!("new replay")
    };
    store
        .record(
            &legs[0].claim,
            &LedgerCompletion::Acknowledged {
                detail: "accepted".into(),
            },
            21,
        )
        .unwrap();
    store
        .record(
            &legs[1].claim,
            &LedgerCompletion::Retry {
                outcome: UnconfirmedDelivery::Failed,
                detail: "refused".into(),
                retry_at: 30,
            },
            21,
        )
        .unwrap();
}
