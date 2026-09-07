use super::*;

fn lock_refusal(failure: LockFailure) {
    let fixture = Fixture::new(vec![LaneReport::new("alpha")]);
    fixture.0.borrow_mut().lock_failure = Some(failure);
    let outcome = fixture.run(&[("alpha", 60)], None);
    assert_eq!(fixture.events(), [Event::Acquire]);
    assert!(!fixture.0.borrow().held);
    match outcome {
        RunOutcome::LockRefused(LockFailure::Contended(reason)) => {
            assert_eq!(reason, "fixture contention")
        }
        RunOutcome::LockRefused(LockFailure::Unavailable(reason)) => {
            assert_eq!(reason, "fixture unavailable")
        }
        other => panic!("lost lock refusal: {other:?}"),
    }
}

#[test]
fn a_contended_lock_refuses_the_run_before_any_other_port_is_called() {
    lock_refusal(LockFailure::Contended("fixture contention".into()));
}

#[test]
fn an_unavailable_lock_refuses_the_run_before_any_other_port_is_called() {
    lock_refusal(LockFailure::Unavailable("fixture unavailable".into()));
}

#[test]
fn a_failed_finish_clock_keeps_the_old_marker_and_names_its_location() {
    let fixture = Fixture::new(vec![LaneReport::new("alpha")]);
    fixture.0.borrow_mut().epochs = [Ok(100), Err(ClockFailure::BeforeEpoch)].into();
    assert_eq!(fixture.run(&[("alpha", 60)], None), RunOutcome::Completed);
    let events = fixture.events();
    assert_eq!(
        &events[events.len() - 3..],
        [
            Event::Epoch(None),
            Event::Notice("clock failed: /fixture/marker".into()),
            Event::Release
        ]
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::MarkerWrite(_)))
    );
}

#[test]
fn no_configured_record_channel_allows_a_clean_run_to_advance_the_marker() {
    let fixture = Fixture::new(vec![LaneReport::new("alpha")]);
    fixture.0.borrow_mut().record = Some(RecordOutcome::NotConfigured);
    assert_eq!(fixture.run(&[("alpha", 60)], None), RunOutcome::Completed);
    let events = fixture.events();
    assert_eq!(
        &events[events.len() - 5..],
        [
            Event::Record(0, 0, "fixture record body".into()),
            Event::Notice("no records".into()),
            Event::Epoch(Some(200)),
            Event::MarkerWrite(200),
            Event::Release
        ]
    );
}

#[test]
fn a_failed_record_keeps_the_marker_even_when_its_alert_is_delivered() {
    let fixture = Fixture::new(vec![LaneReport::new("alpha")]);
    fixture.0.borrow_mut().record = Some(RecordOutcome::Rejected {
        url: "http://127.0.0.1:0/record".into(),
        cause: RecordFailure::NoResponse,
        description: "no response".into(),
    });
    assert_eq!(fixture.run(&[("alpha", 60)], None), RunOutcome::Completed);
    let events = fixture.events();
    assert_eq!(
        &events[events.len() - 4..],
        [
            Event::Record(0, 0, "fixture record body".into()),
            Event::Notice("no response".into()),
            Event::Alert("run".into()),
            Event::Release
        ]
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::MarkerWrite(_)))
    );
}

#[test]
fn a_record_that_cannot_be_signed_keeps_the_marker_without_claiming_delivery() {
    let fixture = Fixture::new(vec![LaneReport::new("alpha")]);
    fixture.0.borrow_mut().record = Some(RecordOutcome::SigningFailed);
    assert_eq!(fixture.run(&[("alpha", 60)], None), RunOutcome::Completed);
    let events = fixture.events();
    assert_eq!(
        &events[events.len() - 3..],
        [
            Event::Record(0, 0, "fixture record body".into()),
            Event::Notice("signing failed".into()),
            Event::Release
        ]
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::MarkerWrite(_)))
    );
}
