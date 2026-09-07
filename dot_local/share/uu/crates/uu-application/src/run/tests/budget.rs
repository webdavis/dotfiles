use super::*;

#[test]
fn name_order_remaining_budget_and_later_lanes_survive_an_earlier_failure() {
    let reports = ["alpha", "beta", "gamma", "zeta"]
        .into_iter()
        .map(|name| {
            let mut report = LaneReport::new(name);
            if name != "beta" {
                report.failed("fixture lane failure".into());
            }
            report
        })
        .collect();
    let fixture = Fixture::new(reports);
    fixture.0.borrow_mut().elapsed = [
        RUN_DEADLINE - Duration::from_secs(2),
        RUN_DEADLINE - Duration::from_secs(1),
        RUN_DEADLINE,
        RUN_DEADLINE + Duration::from_secs(1),
    ]
    .into();
    assert_eq!(
        fixture.run(
            &[("zeta", 10), ("gamma", 10), ("beta", 10), ("alpha", 1)],
            None
        ),
        RunOutcome::Completed
    );
    let calls: Vec<_> = fixture
        .events()
        .into_iter()
        .filter_map(|event| match event {
            Event::Lane(name, budget, deadline, epoch, iso, marker) => Some((
                name,
                budget.as_secs(),
                deadline.as_secs(),
                epoch,
                iso,
                marker,
            )),
            _ => None,
        })
        .collect();
    let marker = Marker::Recorded {
        epoch: 7,
        iso: "previous".into(),
    };
    assert_eq!(
        calls,
        [
            (
                "alpha".into(),
                1,
                1,
                100,
                "epoch-100".into(),
                marker.clone()
            ),
            (
                "beta".into(),
                1,
                10,
                100,
                "epoch-100".into(),
                marker.clone()
            ),
            (
                "gamma".into(),
                0,
                10,
                100,
                "epoch-100".into(),
                marker.clone()
            ),
            ("zeta".into(), 0, 10, 100, "epoch-100".into(), marker),
        ]
    );
    assert!(
        fixture
            .events()
            .contains(&Event::Record(3, 0, 0, "fixture record body".into()))
    );
}

#[test]
fn selecting_one_lane_keeps_the_full_declared_set_for_pruning() {
    let fixture = Fixture::new(vec![LaneReport::new("beta")]);
    assert_eq!(
        fixture.run(&[("beta", 60), ("alpha", 60)], Some("beta")),
        RunOutcome::Completed
    );
    assert_eq!(
        fixture.events()[1],
        Event::Prune(vec!["alpha".into(), "beta".into()])
    );
    let calls: Vec<_> = fixture
        .events()
        .into_iter()
        .filter_map(|event| match event {
            Event::Lane(name, ..) => Some(name),
            _ => None,
        })
        .collect();
    assert_eq!(calls, ["beta"]);
}

#[test]
fn an_undeclared_selected_lane_releases_the_guard_without_recording_a_run() {
    let fixture = Fixture::new(vec![]);
    assert_eq!(
        fixture.run(&[("alpha", 60)], Some("unknown")),
        RunOutcome::UndeclaredLane
    );
    assert_eq!(fixture.events().last(), Some(&Event::Release));
    assert!(fixture.events().iter().all(|event| !matches!(
        event,
        Event::Lane(..) | Event::Render(_) | Event::Record(..) | Event::MarkerWrite(_)
    )));
}
