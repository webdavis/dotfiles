use super::*;

fn failed(name: &str) -> LaneReport {
    let mut report = LaneReport::new(name);
    report.failed("fixture failure".into());
    report
}

#[test]
fn the_report_follows_the_record_and_carries_each_lanes_last_ok_time() {
    let fixture = Fixture::new(vec![LaneReport::new("alpha"), failed("beta")]);
    fixture.0.borrow_mut().last_ok.insert("beta".into(), 50);
    assert_eq!(
        fixture.run(&[("alpha", 60), ("beta", 60)], None),
        RunOutcome::Completed
    );
    let events = fixture.events();
    let recorded = events
        .iter()
        .position(|event| matches!(event, Event::Record(..)))
        .unwrap();
    assert_eq!(
        &events[recorded..],
        [
            Event::Record(1, 0, 0, "fixture record body".into()),
            Event::Notice("posted".into()),
            Event::LastOkWrite("alpha".into(), 100),
            Event::LastOkRead("beta".into()),
            Event::Report(
                "epoch-100".into(),
                vec![
                    ("alpha".into(), Some("epoch-100".into())),
                    ("beta".into(), Some("epoch-50".into())),
                ]
            ),
            Event::Release,
        ]
    );
}

#[test]
fn a_lane_that_never_ran_ok_reports_no_last_ok_time() {
    let fixture = Fixture::new(vec![failed("alpha")]);
    assert_eq!(fixture.run(&[("alpha", 60)], None), RunOutcome::Completed);
    assert!(fixture.events().contains(&Event::Report(
        "epoch-100".into(),
        vec![("alpha".into(), None)]
    )));
}

#[test]
fn an_unreadable_start_clock_never_records_a_last_ok_time() {
    let fixture = Fixture::new(vec![LaneReport::new("alpha")]);
    fixture.0.borrow_mut().epochs = [Err(ClockFailure::BeforeEpoch), Ok(200)].into();
    assert_eq!(fixture.run(&[("alpha", 60)], None), RunOutcome::Completed);
    let events = fixture.events();
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::LastOkWrite(..))),
        "{events:?}"
    );
    assert!(events.contains(&Event::Report(
        "epoch-0".into(),
        vec![("alpha".into(), None)]
    )));
}

#[test]
fn an_undelivered_report_raises_one_report_undelivered_alert_and_leaves_the_marker_to_the_record() {
    let fixture = Fixture::new(vec![LaneReport::new("alpha")]);
    fixture.0.borrow_mut().report_fails = true;
    assert_eq!(fixture.run(&[("alpha", 60)], None), RunOutcome::Completed);
    let events = fixture.events();
    let reported = events
        .iter()
        .position(|event| matches!(event, Event::Report(..)))
        .unwrap();
    assert_eq!(
        &events[reported + 1..],
        [
            Event::Alert(AlarmKind::ReportUndelivered, "run".into()),
            Event::Epoch(Some(200)),
            Event::MarkerWrite(200),
            Event::Release,
        ]
    );
    let summaries = fixture.0.borrow().summaries.clone();
    assert_eq!(summaries.len(), 1, "{summaries:?}");
    assert!(
        summaries[0].contains("fixture report failure"),
        "{summaries:?}"
    );
}
