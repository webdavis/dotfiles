use super::*;
#[test]
fn doctor_shows_delivery_backlog_deadletters_and_recording_gaps_without_changing_send_grade() {
    let history = History {
        health: Ok(crate::DeliveryHealth {
            pending_legs: 3,
            deadlettered_legs: 2,
            growth_streak: 2,
            alarm_generation: Some(8),
            recording_gap: true,
        }),
        ..Default::default()
    };
    let (grade, lines) = report(&history, sent(), Outcome::Signalled(1), Pairing::NoAnswer);
    assert!(lines.iter().any(|line| line.contains(
        "3 pending leg(s), 2 deadlettered, growth streak 2, alarm pending; recording gaps recorded"
    )));
    let (unreadable_grade, lines) = report(
        &History::default(),
        sent(),
        Outcome::Signalled(1),
        Pairing::NoAnswer,
    );
    assert_eq!(grade, unreadable_grade);
    assert!(
        lines.iter().any(
            |line| line.contains("delivery ledger unreadable; backlog and deadletters unknown")
        )
    );
}
