use super::{SqliteStore, state};
use pns_application::DecisionRing;
use pns_application::{DecisionOutcomes, SubmissionIdentity};
use pns_domain::Record;
use pns_domain::routing::{Delivery, Leg, ReportMode};
use pns_domain::{DecisionRequest, EnvironmentSnapshot, EventArgs, Overrides, registry::Registry};

fn identity(request: &str) -> SubmissionIdentity {
    SubmissionIdentity {
        producer: "fixture".into(),
        request_id: request.into(),
    }
}
fn record<T>(now: u64, legs: &[(Leg, Delivery)], use_record: impl FnOnce(&Record) -> T) -> T {
    let event = EventArgs {
        agent: "agent".into(),
        state: "done".into(),
        detail: "private message".into(),
        ..EventArgs::default()
    };
    let overrides = Overrides::default();
    let decision = pns_domain::decide(
        &EnvironmentSnapshot::default(),
        &Registry::new().all(),
        &overrides,
        DecisionRequest {
            silence_policy: pns_domain::SilencePolicy::Respect,
            local_only: false,
            remote_only: false,
            pane: "",
            now_secs: Some(now),
            long_running: false,
            mobile_watch_card: false,
        },
    );
    use_record(&Record {
        event: &event,
        decision: &decision,
        overrides: &overrides,
        legs,
        nag: false,
        permission_mode: "",
        agent_id: "",
        tool_name: "",
    })
}
fn delivered() -> Vec<(Leg, Delivery)> {
    vec![(
        Leg {
            name: "mobile",
            mode: ReportMode::ReportOutcome,
            decorative: true,
        },
        Delivery::Delivered("private gateway receipt".into()),
    )]
}

#[test]
fn a_duplicate_begin_keeps_the_recorded_outcome_and_distinct_submission_keys_stay_distinct() {
    let store = SqliteStore::new(state());
    let first = identity("one");
    record(1, &[], |r| store.begin(&first, r)).unwrap();
    assert!(store.revise(&first, "mobile", &delivered()[0].1).unwrap());
    record(2, &[], |r| store.begin(&first, r)).unwrap();
    let second = SubmissionIdentity {
        producer: "another producer".into(),
        request_id: "one".into(),
    };
    record(1, &[], |r| store.begin(&second, r)).unwrap();
    let failed = vec![(
        delivered()[0].0,
        Delivery::Failed("private failure receipt".into()),
    )];
    assert!(store.revise(&second, "mobile", &failed[0].1).unwrap());
    let expected = format!(
        "{}\n{}\n",
        record(1, &delivered(), crate::decision_codec::line),
        record(1, &failed, crate::decision_codec::line)
    );
    assert_eq!(DecisionRing::read(&store).unwrap().unwrap(), expected);
}

#[test]
fn a_per_leg_revision_keeps_arrival_order_and_uses_the_existing_private_codec() {
    let store = SqliteStore::new(state());
    for n in 0..5 {
        record(n, &[], |r| store.begin(&identity(&n.to_string()), r)).unwrap();
    }
    // A retry no longer has the original policy snapshot. Its new clock must
    // not replace the facts recorded by the initial event.
    assert!(
        store
            .revise(&identity("0"), "mobile", &delivered()[0].1)
            .unwrap()
    );
    let outcome = delivered();
    let expected = (0..5)
        .map(|n| {
            format!(
                "{}\n",
                record(
                    n,
                    if n == 0 { &outcome } else { &[] },
                    crate::decision_codec::line
                )
            )
        })
        .collect::<String>();
    let actual = DecisionRing::read(&store).unwrap().unwrap();
    assert_eq!(actual, expected);
    assert!(!actual.contains("private message") && !actual.contains("private gateway receipt"));
    assert!(
        store
            .revise(
                &identity("0"),
                "new.destination",
                &Delivery::Failed("secret failure".into())
            )
            .unwrap()
    );
    assert!(
        store
            .revise(
                &identity("0"),
                "mobile",
                &Delivery::Unlaunched("secret launch".into())
            )
            .unwrap()
    );
    let after = DecisionRing::read(&store).unwrap().unwrap();
    let first = after.lines().next().unwrap();
    assert_eq!(
        first.split_once(" legs=").unwrap().0,
        actual
            .lines()
            .next()
            .unwrap()
            .split_once(" legs=")
            .unwrap()
            .0
    );
    assert!(first.ends_with("legs=mobile:unlaunched,new.destination:failed"));
    assert_eq!(
        after.lines().skip(1).collect::<Vec<_>>(),
        actual.lines().skip(1).collect::<Vec<_>>()
    );
    assert!(!after.contains("secret"));
    let shared_path = store.state.clone();
    let barrier = std::sync::Barrier::new(2);
    std::thread::scope(|scope| {
        let first = scope.spawn(|| {
            let store = SqliteStore::new(shared_path.clone());
            barrier.wait();
            store.revise(
                &identity("0"),
                "mobile",
                &Delivery::Delivered("private".into()),
            )
        });
        let second = scope.spawn(|| {
            let store = SqliteStore::new(shared_path.clone());
            barrier.wait();
            store.revise(&identity("0"), "new.destination", &Delivery::Silent)
        });
        assert!(first.join().unwrap().unwrap());
        assert!(second.join().unwrap().unwrap());
    });
    let after = DecisionRing::read(&store).unwrap().unwrap();
    assert!(
        after
            .lines()
            .next()
            .unwrap()
            .ends_with("legs=mobile:delivered,new.destination:silent"),
        "independent completed legs must both remain: {after}"
    );
    for destination in ["", "x:y", "x,y", "x=y", "x y", "x\ny", "x\u{1b}y"] {
        assert!(
            store
                .revise(&identity("0"), destination, &Delivery::Silent)
                .is_err(),
            "a destination cannot inject a field: {destination:?}"
        );
        assert_eq!(DecisionRing::read(&store).unwrap().unwrap(), after);
    }
}

#[test]
fn a_late_revision_never_resurrects_a_pruned_decision_or_reorders_remaining_events() {
    let store = SqliteStore::new(state());
    for n in 0..5 {
        record(n, &[], |r| store.begin(&identity(&n.to_string()), r)).unwrap();
    }
    // Unkeyed callers share this same ring and must preserve retained identities.
    record(5, &[], |r| store.record_decision(r)).unwrap();
    assert!(
        !store
            .revise(&identity("0"), "mobile", &delivered()[0].1)
            .unwrap()
    );
    assert!(
        store
            .revise(&identity("1"), "mobile", &delivered()[0].1)
            .unwrap()
    );
    let outcome = delivered();
    let expected = (1..=5)
        .map(|n| {
            format!(
                "{}\n",
                record(
                    n,
                    if n == 1 { &outcome } else { &[] },
                    crate::decision_codec::line
                )
            )
        })
        .collect::<String>();
    assert_eq!(DecisionRing::read(&store).unwrap().unwrap(), expected);
    assert!(
        !store
            .revise(&identity("never existed"), "mobile", &delivered()[0].1)
            .unwrap()
    );
}

#[test]
fn appending_keyed_decisions_preserves_legacy_separator_and_pruning_bytes() {
    for initial in ["", "0 old\r\n1 torn", "0 old\r\n1 closed\r\n"] {
        let path = state();
        std::fs::create_dir(&path).unwrap();
        let comparison = path.join("comparison");
        std::fs::write(&comparison, initial).unwrap();
        std::fs::write(path.join("decisions"), initial).unwrap();
        let store = SqliteStore::for_records(path);
        let _open = store.connect().unwrap();
        assert_eq!(
            DecisionRing::read(&store).unwrap().as_deref(),
            Some(initial)
        );
        for n in 2..8 {
            record(n, &[], |r| {
                crate::append_ring_line(
                    &comparison,
                    &crate::decision_codec::line(r),
                    5,
                    crate::RING_READ_MAX,
                )
                .unwrap();
                store.begin(&identity(&n.to_string()), r).unwrap();
            });
            assert_eq!(
                DecisionRing::read(&store).unwrap().unwrap().as_bytes(),
                std::fs::read(&comparison).unwrap()
            );
        }
        assert!(
            store
                .revise(&identity("7"), "mobile", &delivered()[0].1)
                .unwrap()
        );
    }
}

#[test]
fn a_refused_revision_reports_failure_and_preserves_the_prior_outcome() {
    let mut store = SqliteStore::new(state());
    store.log = store.state.join("recording.log");
    record(1, &[], |r| store.begin(&identity("one"), r)).unwrap();
    let before = DecisionRing::read(&store).unwrap();
    let connection = store.connect().unwrap();
    connection.execute_batch("CREATE TRIGGER refuse_decision BEFORE UPDATE ON decisions BEGIN SELECT RAISE(ABORT, 'fixture refusal'); END;").unwrap();
    assert!(
        store
            .revise(&identity("one"), "mobile", &delivered()[0].1)
            .is_err()
    );
    assert_eq!(DecisionRing::read(&store).unwrap(), before);
    assert_eq!(
        std::fs::read_to_string(&store.log).unwrap(),
        "pns: state error (decision: database refused the operation); recording failed\n"
    );
}

#[test]
fn a_retry_refuses_malformed_duplicate_or_missing_leg_fields_without_changing_the_row() {
    for line in [
        "1 original facts without a legs field\n",
        "1 original legs=hermes:failed,hermes:delivered\n",
        "1 original legs=hermes:invented\n",
        "1 original legs=mobile:delivered legs=hermes:failed\n",
    ] {
        let store = SqliteStore::new(state());
        let key = identity("one");
        record(1, &delivered(), |r| store.begin(&key, r)).unwrap();
        store
            .connect()
            .unwrap()
            .execute("UPDATE decisions SET line = ?1", [line])
            .unwrap();
        assert!(
            store.revise(&key, "mobile", &delivered()[0].1).is_err(),
            "a malformed or missing outcome field cannot be rewritten: {line:?}"
        );
        assert_eq!(DecisionRing::read(&store).unwrap().as_deref(), Some(line));
    }
}
