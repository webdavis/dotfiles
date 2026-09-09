use super::*;
use crate::PreparedSubmission;
use pns_domain::{EventArgs, Overrides, Record};

mod fixtures;
mod retry;
use fixtures::*;

#[test]
fn each_leg_is_written_ahead_and_its_atomic_completion_follows_delivery() {
    for delivery in [
        Delivery::Delivered("accepted".into()),
        Delivery::Failed("timeout".into()),
        Delivery::Unlaunched("missing".into()),
        Delivery::Silent,
    ] {
        let store = Store::default();
        let destinations = destinations(&store, delivery.clone());
        let workflow = SubmissionDelivery {
            ledger: &store,
            decisions: &store,
            destinations: &destinations,
        };
        let result = submit(&workflow, &store).unwrap();
        let Submitted::Attempted { sequence, outcomes } = result else {
            panic!("a new submission was not attempted")
        };
        assert_eq!(sequence, Some(77));
        assert_eq!(
            outcomes,
            submission()
                .legs
                .into_iter()
                .map(|leg| (leg, delivery.clone()))
                .collect::<Vec<_>>()
        );
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
        assert_eq!(
            *store.completed.lock().unwrap(),
            [(0, delivery.clone(), 125), (1, delivery, 125)]
        );
    }
}

#[test]
fn existing_or_conflicting_submissions_never_dispatch_or_rewrite_the_decision() {
    for preparation in [
        Preparation::Existing,
        Preparation::Conflict,
        Preparation::InvalidPlan,
        Preparation::InvalidLease,
    ] {
        let store = Store {
            preparation,
            ..Store::default()
        };
        let destinations = destinations(&store, Delivery::Delivered("accepted".into()));
        let workflow = SubmissionDelivery {
            ledger: &store,
            decisions: &store,
            destinations: &destinations,
        };
        let result = submit(&workflow, &store);
        if matches!(preparation, Preparation::Existing) {
            let Submitted::Existing(record) = result.unwrap() else {
                panic!("duplicate was attempted")
            };
            assert_eq!(record.sequence, 77);
            assert_eq!(record.submission, submission());
        } else {
            assert!(matches!(
                result,
                Err(LedgerFailure::ConflictingSubmission
                    | LedgerFailure::InvalidPlan
                    | LedgerFailure::InvalidLease)
            ));
        }
        assert_eq!(store.steps(), ["prepare"]);
    }
}

#[test]
fn unavailable_write_ahead_storage_reports_the_gap_and_keeps_delivery_fail_open() {
    let store = Store {
        preparation: Preparation::Unavailable,
        ..Store::default()
    };
    let destinations = destinations(&store, Delivery::Delivered("accepted".into()));
    let workflow = SubmissionDelivery {
        ledger: &store,
        decisions: &store,
        destinations: &destinations,
    };
    let Submitted::Attempted { sequence, outcomes } = submit(&workflow, &store).unwrap() else {
        panic!("storage failure blocked delivery")
    };
    assert_eq!(sequence, None);
    assert_eq!(outcomes.len(), 2);
    assert!(
        outcomes
            .iter()
            .all(|(_, outcome)| matches!(outcome, Delivery::Delivered(_)))
    );
    assert!(store.completed.lock().unwrap().is_empty());
    assert_eq!(
        store.steps(),
        [
            "prepare",
            "notice",
            "begin",
            "deliver:alpha",
            "revise:alpha",
            "deliver:beta",
            "revise:beta"
        ]
    );
    assert!(store.notices.lock().unwrap()[0].contains("ledger"));
}

#[test]
fn failed_atomic_recording_keeps_each_verdict_and_attempts_every_leg() {
    let store = Store {
        fail_writes: true,
        ..Store::default()
    };
    let destinations = destinations(&store, Delivery::Delivered("accepted".into()));
    let workflow = SubmissionDelivery {
        ledger: &store,
        decisions: &store,
        destinations: &destinations,
    };
    let Submitted::Attempted { outcomes, .. } = submit(&workflow, &store).unwrap() else {
        panic!("storage failure blocked delivery")
    };
    assert_eq!(outcomes.len(), 2);
    assert!(
        outcomes
            .iter()
            .all(|(_, outcome)| matches!(outcome, Delivery::Delivered(_)))
    );
    assert_eq!(
        store.steps(),
        [
            "prepare",
            "begin",
            "notice",
            "deliver:alpha",
            "ledger:0",
            "notice",
            "deliver:beta",
            "ledger:1",
            "notice"
        ]
    );
    assert_eq!(store.completed.lock().unwrap().len(), 2);
}

fn submit<L, R, D>(
    workflow: &SubmissionDelivery<'_, L, R, D>,
    store: &Store,
) -> Result<Submitted, LedgerFailure>
where
    L: DeliveryLedger + Sync,
    L::Claim: Sync,
    R: DecisionOutcomes + Sync,
    D: NotificationDestination,
{
    let decision = decision();
    let event = EventArgs::default();
    let overrides = Overrides::default();
    let record = Record {
        event: &event,
        decision: &decision,
        overrides: &overrides,
        legs: &[],
        nag: false,
        permission_mode: "original-mode",
        agent_id: "original-agent",
        tool_name: "original-tool",
    };
    workflow.submit(
        &submission(),
        Some(&record),
        lease(),
        &|| Some(125),
        &|message| store.notice(message),
    )
}
