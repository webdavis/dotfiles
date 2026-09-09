use super::*;
#[test]
fn clean_recovery_and_successful_publication_clear_their_markers_best_effort() {
    for refusal in [false, true] {
        let mut f = fixture();
        f.marker_refusal = refusal;
        {
            let mut r = f.recorded.borrow_mut();
            r.readings = vec!["old".into()];
            r.persistence = vec!["baseline_persist".into()];
        }
        assert_eq!(
            execute(&mut f, ["2", "1", "1"], false, true, Publication::Success),
            Ok(())
        );
        let r = f.recorded.borrow();
        assert_eq!(
            r.calls,
            [
                "covered:Readings",
                "clear:Readings",
                "publish",
                "clear:Persistence"
            ]
        );
        assert_eq!(r.baseline, [2, 1, 1]);
        assert!(r.alerts.is_empty());
        assert_eq!(r.readings, if refusal { vec!["old"] } else { vec![] });
        assert_eq!(
            r.persistence,
            if refusal {
                vec!["baseline_persist"]
            } else {
                vec![]
            }
        );
    }
}
#[test]
fn publication_failure_keeps_the_captured_file_outcome_and_pages_its_independent_gap() {
    for publication in [Publication::BeforeRename, Publication::AfterRename] {
        let mut f = fixture();
        assert_eq!(
            execute(&mut f, ["2", "1", "1"], false, true, publication),
            Err(PollFailure::Persistence)
        );
        let r = f.recorded.borrow();
        assert_eq!(
            r.calls,
            [
                "covered:Readings",
                "clear:Readings",
                "publish",
                "covered:Persistence",
                "submit:gap",
                "remember:Persistence:baseline_persist"
            ]
        );
        assert_eq!(
            r.baseline,
            if matches!(publication, Publication::BeforeRename) {
                [1, 1, 1]
            } else {
                [2, 1, 1]
            }
        );
        assert_eq!(r.persistence, ["baseline_persist"]);
        assert_eq!(r.alerts.len(), 1);
        assert_eq!(
            r.alerts[0].detail,
            include_str!("persistence-gap.txt").trim_end_matches('\n')
        );
        assert_eq!(r.alerts[0].title, "🔴 **CRITICAL**");
        assert_eq!(r.alerts[0].signal, AlertSignal::NeedsAttention);
    }
}
#[test]
fn persistence_gap_refusal_keeps_coverage_and_an_already_covered_gap_only_refreshes() {
    for failure in FAILURES {
        for covered in [false, true] {
            let mut f = fixture();
            f.recorded.borrow_mut().persistence = if covered {
                vec!["baseline_persist".into(), "old".into()]
            } else {
                vec!["old".into()]
            };
            f.submissions.push_back(Submission::NotAccepted(failure));
            assert_eq!(
                execute(
                    &mut f,
                    ["2", "1", "1"],
                    false,
                    true,
                    Publication::BeforeRename
                ),
                Err(PollFailure::Persistence)
            );
            let r = f.recorded.borrow();
            let mut expected = vec![
                "covered:Readings",
                "clear:Readings",
                "publish",
                "covered:Persistence",
            ];
            expected.push(if covered {
                "remember:Persistence:baseline_persist"
            } else {
                "submit:gap"
            });
            assert_eq!(r.calls, expected);
            assert_eq!(
                r.persistence,
                if covered {
                    vec!["baseline_persist"]
                } else {
                    vec!["old"]
                }
            );
            assert_eq!(r.alerts.len(), usize::from(!covered));
            assert_eq!(r.baseline, [1, 1, 1]);
        }
    }
}
