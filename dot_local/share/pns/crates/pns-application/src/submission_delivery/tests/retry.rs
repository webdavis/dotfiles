use super::*;
use crate::RetryDelivery;
use std::sync::Mutex;

#[test]
fn a_daemon_retry_carries_the_original_payload_route_identity_and_claim_once() {
    let original = submission();
    let leg = original.legs[1].clone();
    let store = Store {
        allow_retry: true,
        retry: Mutex::new(Some(RetryDelivery {
            claim: 901,
            identity: original.identity,
            event: original.event,
            leg: leg.clone(),
        })),
        ..Store::default()
    };
    let registry = destinations(&store, Delivery::Delivered("accepted".into()));
    let workflow = SubmissionDelivery {
        ledger: &store,
        decisions: &store,
        destinations: &registry,
    };
    assert_eq!(
        workflow
            .retry(lease(), &|| Some(125), &|message| store.notice(message))
            .unwrap(),
        Some((leg, Delivery::Delivered("accepted".into())))
    );
    assert_eq!(store.steps(), ["claim", "deliver:beta", "ledger:901"]);
    assert_eq!(
        *store.completed.lock().unwrap(),
        [(
            901,
            LedgerCompletion::Acknowledged {
                detail: "accepted".into()
            },
            125
        )]
    );
    assert_eq!(
        workflow
            .retry(lease(), &|| Some(125), &|message| store.notice(message))
            .unwrap(),
        None
    );
    assert_eq!(
        store.steps(),
        ["claim", "deliver:beta", "ledger:901", "claim"]
    );
}

#[test]
fn a_missing_retry_destination_is_recorded_unlaunched_without_an_inline_retry() {
    let original = submission();
    let store = Store {
        allow_retry: true,
        retry: Mutex::new(Some(RetryDelivery {
            claim: 902,
            identity: original.identity,
            event: original.event,
            leg: original.legs[0].clone(),
        })),
        ..Store::default()
    };
    let registry: Destinations<Destination<'_>> = Destinations::new();
    let workflow = SubmissionDelivery {
        ledger: &store,
        decisions: &store,
        destinations: &registry,
    };
    let (_, delivery) = workflow
        .retry(lease(), &|| None, &|message| store.notice(message))
        .unwrap()
        .unwrap();
    assert!(matches!(&delivery, Delivery::Unlaunched(reason) if reason.contains("not registered")));
    assert_eq!(store.steps(), ["claim", "ledger:902"]);
    let completions = store.completed.lock().unwrap();
    assert!(matches!(
        &completions[0],
        (
            902,
            LedgerCompletion::Retry {
                outcome: UnconfirmedDelivery::Unlaunched,
                retry_at: 130,
                ..
            },
            100
        )
    ));
}

#[test]
fn every_registered_destination_is_guarded_and_recorded_even_when_it_panics() {
    let store = Store {
        panic_on_delivery: true,
        ..Store::default()
    };
    let registry = destinations(&store, Delivery::Silent);
    let workflow = SubmissionDelivery {
        ledger: &store,
        decisions: &store,
        destinations: &registry,
    };
    let Submitted::Attempted { outcomes, .. } = submit(&workflow, &store).unwrap() else {
        panic!("new request was not attempted")
    };
    assert_eq!(outcomes.len(), 2);
    assert!(outcomes.iter().all(|(_, result)| matches!(result, Delivery::Failed(detail) if !detail.contains("private transport panic"))));
    assert_eq!(
        store.steps(),
        [
            "prepare",
            "begin",
            "deliver:alpha",
            "ledger:0",
            "deliver:beta",
            "ledger:1"
        ]
    );
    let completed = store.completed.lock().unwrap();
    assert_eq!(completed.len(), 2);
    assert!(completed.iter().all(|(_, completion, _)| matches!(
        completion,
        LedgerCompletion::Retry {
            outcome: UnconfirmedDelivery::Failed,
            ..
        }
    )));
}

#[test]
fn an_aggregate_replay_is_queued_without_creating_an_original_decision() {
    let store = Store::default();
    let destinations = destinations(&store, Delivery::Delivered("accepted".into()));
    let workflow = SubmissionDelivery {
        ledger: &store,
        decisions: &store,
        destinations: &destinations,
    };
    let result = workflow
        .submit(&submission(), None, lease(), &|| Some(125), &|message| {
            store.notice(message)
        })
        .unwrap();
    let Submitted::Attempted { sequence, outcomes } = result else {
        panic!("new replay was not attempted")
    };
    assert_eq!(sequence, Some(77));
    assert_eq!(outcomes.len(), 2);
    assert_eq!(
        store.steps(),
        [
            "prepare",
            "deliver:alpha",
            "ledger:0",
            "deliver:beta",
            "ledger:1"
        ]
    );
    assert_eq!(store.completed.lock().unwrap().len(), 2);
}
