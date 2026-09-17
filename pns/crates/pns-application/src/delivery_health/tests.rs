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

/// The pointer is what turns a count into a next step, and it appears only when
/// there is something to look at.
#[test]
fn the_summary_names_the_detail_view_exactly_when_something_is_not_arriving() {
    let healthy = DeliveryHealth {
        pending_legs: 0,
        deadlettered_legs: 0,
        ..health()
    };
    assert!(!mentions_failures(&delivery_health_lines(Ok(
        healthy.clone()
    ))));

    let pending = DeliveryHealth {
        pending_legs: 1,
        ..healthy.clone()
    };
    // THE POINTER IS THE LAST LINE, so it reads as the section's next step
    // rather than as one more finding among the counts above it.
    let pending = delivery_health_lines(Ok(pending));
    assert_eq!(
        pending.last().map(|(_, line)| line.as_str()),
        Some("pns doctor: run `pns failures` for what is not arriving"),
        "{pending:?}"
    );

    let deadlettered = DeliveryHealth {
        deadlettered_legs: 2,
        ..healthy
    };
    assert!(mentions_failures(&delivery_health_lines(Ok(deadlettered))));
}

/// An unreadable ledger says so and points nowhere: there is no count behind
/// the pointer, so offering it would send the reader to an empty listing.
#[test]
fn an_unreadable_ledger_names_no_detail_view() {
    assert!(!mentions_failures(&delivery_health_lines(Err(
        "gone".into()
    ))));
}

/// Whether the section pointed the reader at the detail view, wherever in it
/// the pointer landed.
fn mentions_failures(lines: &[(pns_domain::doctor::Mark, String)]) -> bool {
    lines.iter().any(|(_, line)| line.contains("pns failures"))
}

/// A count is a finding, not a reading: the doctor's summary withholds its
/// all-clear on a warning, so a backlog or a dead letter has to arrive marked
/// as one.
#[test]
fn every_count_and_every_fault_is_marked_a_warning_and_the_healthy_sentence_is_not() {
    use pns_domain::doctor::Mark;
    let quiet = DeliveryHealth {
        pending_legs: 0,
        deadlettered_legs: 0,
        growth_streak: 0,
        alarm_generation: None,
        recording_gap: false,
    };
    assert_eq!(
        delivery_health_lines(Ok(quiet.clone())),
        vec![(
            Mark::Good,
            "pns doctor: nothing is queued and nothing was given up on".to_string()
        )]
    );

    for health in [
        DeliveryHealth {
            pending_legs: 1,
            ..quiet.clone()
        },
        DeliveryHealth {
            deadlettered_legs: 17,
            ..quiet.clone()
        },
        DeliveryHealth {
            growth_streak: 1,
            ..quiet.clone()
        },
        DeliveryHealth {
            alarm_generation: Some(3),
            ..quiet.clone()
        },
        DeliveryHealth {
            recording_gap: true,
            ..quiet.clone()
        },
    ] {
        let lines = delivery_health_lines(Ok(health.clone()));
        assert_eq!(lines[0].0, Mark::Warn, "{health:?} -> {lines:?}");
    }

    // The pointer is the section's next step, not a second finding.
    let pointing = delivery_health_lines(Ok(DeliveryHealth {
        deadlettered_legs: 2,
        ..quiet
    }));
    assert_eq!(pointing.last().unwrap().0, Mark::Detail, "{pointing:?}");

    // An unreadable record is a warning too: the counts behind it are unknown,
    // which is not the same as zero.
    assert_eq!(delivery_health_lines(Err("gone".into()))[0].0, Mark::Warn);
}
