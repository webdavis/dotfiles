use super::*;
#[test]
fn a_new_gap_is_accepted_and_marked_before_an_independent_exposure_and_baseline() {
    let mut f = fixture();
    assert_eq!(
        execute(&mut f, ["0", "1", "1"], true, false, Publication::Success),
        Ok(())
    );
    let r = f.recorded.borrow();
    assert_eq!(
        r.calls,
        [
            "covered:Readings",
            "submit:gap",
            "remember:Readings:controls_file",
            "submit:page",
            "publish",
            "clear:Persistence"
        ]
    );
    assert_eq!(r.readings, ["controls_file"]);
    assert_eq!(r.baseline, [0, 1, 1]);
    assert_eq!(r.alerts.len(), 2);
    for alert in &r.alerts {
        assert_eq!(alert.signal, AlertSignal::NeedsAttention);
        assert_eq!(alert.occurred_at, Some(42));
        assert_eq!(alert.occurrence_id, None);
        assert_eq!(alert.title, "🔴 **CRITICAL**");
    }
    assert!(
        r.alerts[0]
            .detail
            .starts_with("**Security-posture monitoring gap**")
    );
    assert!(
        r.alerts[1]
            .detail
            .starts_with("**Firewall is OFF (first observation)**")
    );
}
#[test]
fn a_refused_gap_advances_no_marker_exposure_or_baseline() {
    for failure in FAILURES {
        let mut f = fixture();
        f.recorded.borrow_mut().readings = vec!["old".into()];
        f.submissions.push_back(Submission::NotAccepted(failure));
        assert_eq!(
            execute(&mut f, ["0", "1", "1"], true, true, Publication::Success),
            Err(PollFailure::Gap(failure))
        );
        let r = f.recorded.borrow();
        assert_eq!(r.calls, ["covered:Readings", "submit:gap"]);
        assert_eq!(r.readings, ["old"]);
        assert_eq!(r.baseline, [1, 1, 1]);
    }
}
#[test]
fn an_already_covered_gap_refreshes_current_members_even_when_marker_writes_refuse() {
    for refusal in [false, true] {
        let mut f = fixture();
        f.marker_refusal = refusal;
        f.recorded.borrow_mut().readings = vec!["controls_file".into(), "old".into()];
        assert_eq!(
            execute(&mut f, ["1", "1", "1"], true, false, Publication::Success),
            Ok(())
        );
        let r = f.recorded.borrow();
        assert_eq!(
            r.calls,
            [
                "covered:Readings",
                "remember:Readings:controls_file",
                "publish",
                "clear:Persistence"
            ]
        );
        assert_eq!(
            r.readings,
            if refusal {
                vec!["controls_file", "old"]
            } else {
                vec!["controls_file"]
            }
        );
        assert!(r.alerts.is_empty());
    }
}
#[test]
fn an_unreadable_trio_without_prior_stops_after_the_accepted_gap() {
    let mut f = fixture();
    f.recorded.borrow_mut().persistence = vec!["baseline_persist".into()];
    assert_eq!(
        execute(&mut f, ["", "1", "1"], false, false, Publication::Success),
        Ok(())
    );
    let r = f.recorded.borrow();
    assert_eq!(
        r.calls,
        [
            "covered:Readings",
            "submit:gap",
            "remember:Readings:posture_query"
        ]
    );
    assert_eq!(r.readings, ["posture_query"]);
    assert_eq!(r.persistence, ["baseline_persist"]);
    assert_eq!(r.baseline, [1, 1, 1]);
}
#[test]
fn exposure_refusal_preserves_baseline_after_the_independent_gap_was_accepted() {
    for failure in FAILURES {
        let mut f = fixture();
        f.submissions = VecDeque::from([Submission::Accepted, Submission::NotAccepted(failure)]);
        assert_eq!(
            execute(&mut f, ["0", "1", "1"], true, true, Publication::Success),
            Err(PollFailure::Exposure(failure))
        );
        let r = f.recorded.borrow();
        assert_eq!(
            r.calls,
            [
                "covered:Readings",
                "submit:gap",
                "remember:Readings:controls_file",
                "submit:page"
            ]
        );
        assert_eq!(r.readings, ["controls_file"]);
        assert_eq!(r.baseline, [1, 1, 1]);
    }
}
