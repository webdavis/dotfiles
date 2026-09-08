use super::*;
use std::cell::Cell;
fn health() -> DeliveryHealth {
    DeliveryHealth {
        pending_legs: 2,
        deadlettered_legs: 1,
        growth_streak: 2,
        alarm_generation: Some(7),
        recording_gap: true,
    }
}
#[test]
fn delivery_alarm_is_acknowledged_only_after_a_confirmed_banner() {
    for outcome in [
        Delivery::Silent,
        Delivery::Failed("no banner".into()),
        Delivery::Unlaunched("missing".into()),
    ] {
        assert!(
            report_delivery_health(
                Ok(&health()),
                |_| outcome,
                |_| panic!("unconfirmed alarm must remain pending")
            )
            .is_err()
        );
    }
    let sent = Cell::new(false);
    report_delivery_health(
        Ok(&health()),
        |message| {
            assert!(message.contains("2 undelivered") && message.contains("1 deadlettered"));
            sent.set(true);
            Delivery::Delivered("banner".into())
        },
        |generation| {
            assert!(sent.get());
            assert_eq!(generation, 7);
            Ok(())
        },
    )
    .unwrap();
}
#[test]
fn delivery_health_never_reports_unreadable_storage_or_failed_acknowledgement_as_healthy() {
    let calls = Cell::new(0);
    let missing = LedgerFailure::Unavailable("private storage detail".into());
    for _ in 0..2 {
        assert!(
            report_delivery_health(
                Err(&missing),
                |message| {
                    assert!(message.contains("unreadable"));
                    assert!(!message.contains("private storage detail"));
                    calls.set(calls.get() + 1);
                    Delivery::Delivered("banner".into())
                },
                |_| panic!("unreadable storage has no acknowledgement")
            )
            .is_err()
        );
    }
    assert_eq!(
        calls.get(),
        2,
        "failed storage is eligible again on the next tick"
    );
    assert!(
        report_delivery_health(
            Ok(&health()),
            |_| Delivery::Delivered("banner".into()),
            |_| Err(LedgerFailure::Unavailable("ack failed".into()))
        )
        .is_err()
    );
    let mut quiet = health();
    quiet.alarm_generation = None;
    report_delivery_health(
        Ok(&quiet),
        |_| panic!("no new or pending alarm"),
        |_| panic!("no alarm to acknowledge"),
    )
    .unwrap();
}
