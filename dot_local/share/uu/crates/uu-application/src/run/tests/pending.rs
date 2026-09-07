use super::*;
use std::num::NonZeroU32;

fn pending() -> LaneReport {
    let mut report = LaneReport::new("alpha");
    report.pending("approval required".into());
    report
}

#[test]
fn pending_runs_count_separately_and_advance_the_success_marker() {
    let fixture = Fixture::new(vec![pending()]);
    fixture.0.borrow_mut().streaks.insert("alpha".into(), 2);
    assert_eq!(fixture.run(&[("alpha", 60)], None), RunOutcome::Completed);
    assert_eq!(fixture.0.borrow().pending["alpha"], 1);
    assert_eq!(fixture.0.borrow().streaks["alpha"], 0);
    assert!(
        fixture
            .events()
            .contains(&Event::Record(0, 0, 1, "fixture record body".into()))
    );
    assert!(fixture.events().contains(&Event::MarkerWrite(200)));
    assert!(
        !fixture
            .events()
            .iter()
            .any(|event| matches!(event, Event::Alert(..)))
    );
}

#[test]
fn the_configured_pending_threshold_alerts_once_and_before_its_count_is_published() {
    for (previous, alert) in [(0, false), (1, true), (2, false)] {
        let fixture = Fixture::new(vec![pending()]);
        fixture
            .0
            .borrow_mut()
            .pending
            .insert("alpha".into(), previous);
        assert_eq!(
            fixture.run_with_threshold(&[("alpha", 60)], None, NonZeroU32::new(2).unwrap()),
            RunOutcome::Completed
        );
        let events = fixture.events();
        let alarm = events
            .iter()
            .position(|event| *event == Event::Alert(AlarmKind::Pending, "alpha".into()));
        assert_eq!(alarm.is_some(), alert, "previous {previous}: {events:?}");
        let write = events
            .iter()
            .position(|event| {
                *event == Event::StreakWrite(StreakKind::Pending, "alpha".into(), previous + 1)
            })
            .unwrap();
        if let Some(alarm) = alarm {
            assert!(alarm < write);
        }
    }
}

#[test]
fn a_refused_pending_alarm_stays_retryable_without_blocking_a_clean_record_marker() {
    let fixture = Fixture::new(vec![pending()]);
    fixture.0.borrow_mut().pending.insert("alpha".into(), 2);
    fixture.0.borrow_mut().fail_alerts = true;
    assert_eq!(fixture.run(&[("alpha", 60)], None), RunOutcome::Completed);
    assert_eq!(fixture.0.borrow().pending["alpha"], 2);
    assert!(fixture.events().contains(&Event::MarkerWrite(200)));
    let next = Fixture::new(vec![pending()]);
    next.0.borrow_mut().pending = fixture.0.borrow().pending.clone();
    assert_eq!(next.run(&[("alpha", 60)], None), RunOutcome::Completed);
    assert!(
        next.events()
            .contains(&Event::Alert(AlarmKind::Pending, "alpha".into()))
    );
    assert_eq!(next.0.borrow().pending["alpha"], 3);
}

#[test]
fn completed_failed_and_deferred_verdicts_all_break_the_pending_streak() {
    for kind in ["completed", "failed", "deferred"] {
        let mut report = LaneReport::new("alpha");
        match kind {
            "failed" => report.failed("failed".into()),
            "deferred" => report.deferred("deferred".into()),
            _ => {}
        }
        let fixture = Fixture::new(vec![report]);
        fixture.0.borrow_mut().pending.insert("alpha".into(), 2);
        assert_eq!(fixture.run(&[("alpha", 60)], None), RunOutcome::Completed);
        assert_eq!(fixture.0.borrow().pending["alpha"], 0, "{kind}");
        assert!(
            !fixture
                .events()
                .contains(&Event::Alert(AlarmKind::Pending, "alpha".into()))
        );
    }
}

#[test]
fn unreadable_pending_history_is_reported_and_not_silently_reset() {
    let fixture = Fixture::new(vec![pending()]);
    fixture.0.borrow_mut().unreadable_pending = true;
    assert_eq!(fixture.run(&[("alpha", 60)], None), RunOutcome::Completed);
    assert_eq!(fixture.0.borrow().pending["alpha"], 3);
    let data = fixture.0.borrow();
    assert!(data.summaries.iter().any(
        |text| text.contains("could not be trusted") && text.contains("fixture damaged count")
    ));
    assert!(
        data.summaries
            .iter()
            .any(|text| text.contains("3 consecutive"))
    );
}

#[test]
fn a_refused_pending_count_write_is_visible_and_alerted() {
    let fixture = Fixture::new(vec![pending()]);
    fixture.0.borrow_mut().fail_pending_write = true;
    assert_eq!(fixture.run(&[("alpha", 60)], None), RunOutcome::Completed);
    assert!(fixture.0.borrow().pending.is_empty());
    assert!(fixture.events().contains(&Event::Notice(
        "Pending streak write failed: fixture write refused".into()
    )));
    assert!(fixture.0.borrow().summaries.iter().any(
        |text| text.contains("could not be recorded") && text.contains("fixture write refused")
    ));
}
