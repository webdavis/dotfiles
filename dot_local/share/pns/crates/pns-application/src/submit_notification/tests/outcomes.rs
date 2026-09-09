use super::{
    Recorder, Submission, SubmissionIdentity, delivered_decision, event, missed_decision, ports,
    submission,
};
use pns_domain::Overrides;
use pns_domain::routing::{Delivery, Leg, ReportMode};

fn leg(decorative: bool) -> Leg {
    Leg {
        name: "other-surface",
        mode: ReportMode::Silent,
        decorative,
    }
}

#[test]
fn the_journal_receives_the_original_identity_event_and_clock() {
    for now in [None, Some(37), Some(u64::MAX)] {
        let (event, overrides) = (event(), Overrides::default());
        let mut decision = missed_decision();
        decision.inputs.now_secs = now;
        let identity = SubmissionIdentity {
            producer: "original-producer".into(),
            request_id: "original-request-id".into(),
        };
        let taken = Submission {
            identity: Some(&identity),
            ..submission(&event, &decision, &overrides)
        };
        let recorder = Recorder::default();
        ports(&recorder).record(&taken);
        assert_eq!(
            *recorder.journaled.borrow(),
            [(std::ptr::from_ref(&event), now, Some(identity))]
        );
    }
}

#[test]
fn an_unconfirmed_decoration_journals_and_passes_the_same_miss_to_lamps() {
    let (event, decision, overrides) = (event(), delivered_decision(), Overrides::default());
    for outcome in [
        Delivery::Failed("refused".into()),
        Delivery::Unlaunched("absent".into()),
        Delivery::Silent,
    ] {
        let outcomes = [(leg(true), outcome)];
        let taken = Submission {
            legs: &outcomes,
            ..submission(&event, &decision, &overrides)
        };
        let recorder = Recorder::default();
        ports(&recorder).record(&taken);
        assert_eq!(recorder.journaled.borrow().len(), 1);
        assert_eq!(*recorder.misses.borrow(), [true]);
    }
}

#[test]
fn an_acknowledged_decoration_skips_the_journal_and_passes_no_miss_to_lamps() {
    let (event, decision, overrides) = (event(), missed_decision(), Overrides::default());
    let outcomes = [(leg(true), Delivery::Delivered("accepted".into()))];
    let taken = Submission {
        legs: &outcomes,
        ..submission(&event, &decision, &overrides)
    };
    let recorder = Recorder::default();
    ports(&recorder).record(&taken);
    assert!(recorder.journaled.borrow().is_empty());
    assert_eq!(*recorder.misses.borrow(), [false]);
}

#[test]
fn durable_delivery_keeps_the_same_unread_lamp_lease_as_the_journal() {
    let (event, decision, overrides) = (event(), delivered_decision(), Overrides::default());
    let outcomes = [(leg(false), Delivery::Delivered("logged".into()))];
    let taken = Submission {
        legs: &outcomes,
        ..submission(&event, &decision, &overrides)
    };
    let recorder = Recorder::default();
    ports(&recorder).record(&taken);
    assert_eq!(recorder.journaled.borrow().len(), 1);
    assert_eq!(*recorder.misses.borrow(), [true]);
}
