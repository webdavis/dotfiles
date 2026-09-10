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
    // ONE FACT PER LINE, all five present. They used to be one sentence in the
    // ledger's own vocabulary, which a reader met as five nouns they had no
    // model for and skimmed as a unit.
    for expected in [
        "3 notifications still waiting to reach a channel",
        "2 notifications given up on after retrying",
        "the backlog has grown 2 checks in a row",
        "an alarm about this has not reached you yet",
        "failed to record a delivery",
    ] {
        assert!(
            lines.iter().any(|line| line.contains(expected)),
            "{expected:?} missing from {lines:?}"
        );
    }
    let (unreadable_grade, lines) = report(
        &History::default(),
        sent(),
        Outcome::Signalled(1),
        Pairing::NoAnswer,
    );
    assert_eq!(grade, unreadable_grade);
    assert!(lines.iter().any(|line| {
        line.contains("the delivery record could not be read, so nothing here is known")
    }));
}
