use super::*;

#[test]
fn a_whole_run_samples_old_facts_and_holds_the_guard_through_marker_publication() {
    let report = LaneReport::new("alpha");
    let fixture = Fixture::new(vec![report.clone()]);
    assert_eq!(fixture.run(&[("alpha", 60)], None), RunOutcome::Completed);
    let marker = Marker::Recorded {
        epoch: 7,
        iso: "previous".into(),
    };
    assert_eq!(
        fixture.events(),
        [
            Event::Acquire,
            Event::Prune(vec!["alpha".into()]),
            Event::Epoch(Some(100)),
            Event::MarkerRead,
            Event::Header(100, marker.clone()),
            Event::Start,
            Event::Elapsed(Duration::ZERO),
            Event::Lane(
                "alpha".into(),
                Duration::from_secs(60),
                Duration::from_secs(60),
                100,
                "epoch-100".into(),
                marker
            ),
            Event::Render(vec![report]),
            Event::StreakRead("alpha".into()),
            Event::StreakWrite("alpha".into(), 0),
            Event::Record(0, 0, "fixture record body".into()),
            Event::Notice("posted".into()),
            Event::Epoch(Some(200)),
            Event::MarkerWrite(200),
            Event::Release,
        ]
    );
    assert!(!fixture.0.borrow().held);
}

#[test]
fn a_lane_failure_is_alerted_before_streak_and_record_publication() {
    let mut report = LaneReport::new("alpha");
    report.failed("fixture failure".into());
    let fixture = Fixture::new(vec![report.clone()]);
    assert_eq!(fixture.run(&[("alpha", 60)], None), RunOutcome::Completed);
    let events = fixture.events();
    let rendered = events
        .iter()
        .position(|event| matches!(event, Event::Render(_)))
        .unwrap();
    assert_eq!(
        &events[rendered..],
        [
            Event::Render(vec![report]),
            Event::Alert("alpha".into()),
            Event::StreakRead("alpha".into()),
            Event::StreakWrite("alpha".into(), 1),
            Event::Record(1, 0, "fixture record body".into()),
            Event::Notice("posted".into()),
            Event::Release,
        ]
    );
}

#[test]
fn a_failed_stale_alert_is_attempted_before_publishing_the_retry_streak() {
    let mut report = LaneReport::new("alpha");
    report.deferred("fixture deferral".into());
    let fixture = Fixture::new(vec![report]);
    fixture.0.borrow_mut().streaks.insert("alpha".into(), 2);
    fixture.0.borrow_mut().fail_alerts = true;
    assert_eq!(fixture.run(&[("alpha", 60)], None), RunOutcome::Completed);
    let events = fixture.events();
    let read = events
        .iter()
        .position(|event| matches!(event, Event::StreakRead(_)))
        .unwrap();
    assert_eq!(
        &events[read..],
        [
            Event::StreakRead("alpha".into()),
            Event::Alert("alpha".into()),
            Event::Notice("alert failed".into()),
            Event::StreakWrite("alpha".into(), 2),
            Event::Record(0, 1, "fixture record body".into()),
            Event::Notice("posted".into()),
            Event::Release,
        ]
    );
    assert_eq!(fixture.0.borrow().streaks["alpha"], 2);
}
