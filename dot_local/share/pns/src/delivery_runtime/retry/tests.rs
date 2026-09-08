use super::*;
use pns_application::{
    LedgerCompletion, LedgerLeg, LedgerSubmission, PreparedSubmission, SubmissionIdentity,
};
use pns_domain::{Delivery, EventArgs, retry::RetryLimits, routing::ReportMode};
use std::cell::Cell;

fn input(id: &str, route: &str) -> LedgerSubmission {
    LedgerSubmission {
        identity: SubmissionIdentity {
            producer: "posture".into(),
            request_id: id.into(),
        },
        producer_request: Some("retained canonical metadata".into()),
        event: channel_dispatch::rendered_event_quiet(
            &EventArgs {
                detail: "original body".into(),
                ..Default::default()
            },
            false,
        ),
        legs: vec![LedgerLeg {
            destination: "hermes".into(),
            route: route.into(),
            mode: ReportMode::Silent,
            decorative: false,
        }],
    }
}
#[test]
fn daemon_retry_uses_limits_and_retained_route_and_continues_after_a_failed_health_banner() {
    let state = crate::runtime_test_support::scratch("retry-health");
    let store = SqliteStore::new(state);
    store
        .prepare(
            &input("old", "old-route"),
            pns_application::LeaseWindow { now: 1, until: 2 },
        )
        .unwrap();
    let later = input("later", "retained-priority");
    store
        .prepare(&later, pns_application::LeaseWindow { now: 3, until: 4 })
        .unwrap();
    let calls = Cell::new(0);
    let limits = RetryLimits {
        max_attempts: 20,
        max_age_secs: 10,
    };
    let result = retry_once(
        &store,
        12,
        limits,
        |message| {
            assert!(message.contains("1 deadlettered"));
            calls.set(1);
            Delivery::Failed("fixture".into())
        },
        |retry, _| {
            assert_eq!(calls.get(), 1, "alarm attempt precedes retry transport");
            assert_eq!(retry.identity, later.identity);
            assert_eq!(retry.event, later.event);
            assert_eq!(retry.leg.route, "retained-priority");
            store
                .record(
                    &retry.claim,
                    &LedgerCompletion::Acknowledged {
                        detail: "fixture accepted".into(),
                    },
                    12,
                )
                .unwrap();
            calls.set(2);
        },
    );
    assert!(result.is_err());
    assert_eq!(calls.get(), 2);
    assert!(store.delivery_health().unwrap().alarm_generation.is_some());
    retry_once(
        &store,
        13,
        limits,
        |_| Delivery::Delivered("banner".into()),
        |_, _| panic!("only deadletters and acknowledged legs remain"),
    )
    .unwrap();
    assert_eq!(store.delivery_health().unwrap().alarm_generation, None);
    assert!(matches!(
        store.prepare(&later, lease(20).unwrap()).unwrap(),
        PreparedSubmission::Existing(_)
    ));
}
#[test]
fn daemon_retry_missing_storage_alarms_without_creating_a_healthy_empty_ledger() {
    let state = crate::runtime_test_support::scratch("retry-missing").join("absent");
    let store = SqliteStore::new(state.clone());
    let calls = Cell::new(0);
    for _ in 0..2 {
        assert!(
            retry_once(
                &store,
                12,
                Default::default(),
                |message| {
                    assert!(message.contains("unreadable"));
                    calls.set(calls.get() + 1);
                    Delivery::Delivered("banner".into())
                },
                |_, _| panic!("no retained row can be dispatched")
            )
            .is_err()
        );
    }
    assert_eq!(calls.get(), 2);
    assert!(!state.exists());
}
#[test]
fn daemon_health_banner_uses_the_native_sound_enabled_adapter_and_observes_its_failure() {
    struct Runner(bool);
    impl pns_application::CommandRunner for Runner {
        fn run(&self, program: &str, arguments: &[&str]) -> Option<String> {
            assert_eq!(program, "terminal-notifier");
            assert!(
                arguments
                    .windows(2)
                    .any(|pair| pair == ["-sound", "default"])
            );
            assert!(
                arguments
                    .iter()
                    .any(|arg| arg.contains("delivery pipeline degraded"))
            );
            self.0.then(String::new)
        }
    }
    assert!(matches!(
        alarm_banner("one undelivered page", Runner(true)),
        Delivery::Delivered(_)
    ));
    assert!(matches!(
        alarm_banner("one undelivered page", Runner(false)),
        Delivery::Failed(_)
    ));
}
