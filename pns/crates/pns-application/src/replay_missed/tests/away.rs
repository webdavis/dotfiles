use super::*;

/// The shortest absence these tests treat as worth a recap.
const MINIMUM_AWAY: u64 = 600;

/// `policy()`, with the time bar raised to `MINIMUM_AWAY`.
fn away_policy() -> RecapPolicy {
    RecapPolicy {
        minimum_away: std::time::Duration::from_secs(MINIMUM_AWAY),
        ..policy()
    }
}

#[test]
fn a_loud_window_shorter_than_the_minimum_away_publishes_no_digest() {
    let mut recorder = Recorder::new(Some(claim_of(
        Some(2_000 - MINIMUM_AWAY + 1),
        vec![entry(1_500)],
    )));
    recorder.entries = vec![entry(1_600), entry(1_700)];
    ports(&recorder).run(&returning(vec![leg(true)]), away_policy(), true);
    assert!(
        recorder.publications.borrow().is_empty(),
        "{:?}",
        recorder.steps()
    );
    // WHAT IS WAITING STILL REACHES THE OPERATOR, as the card with no digest.
    assert_eq!(
        *recorder.delivered.borrow(),
        ["1 missed notification. claude · stop: did a thing"]
    );
}

#[test]
fn an_absence_of_exactly_the_minimum_away_publishes_the_digest() {
    let mut recorder = Recorder::new(Some(claim_of(
        Some(2_000 - MINIMUM_AWAY),
        vec![entry(1_500)],
    )));
    recorder.entries = vec![entry(1_600), entry(1_700)];
    ports(&recorder).run(&returning(vec![leg(true)]), away_policy(), true);
    assert_eq!(
        *recorder.publications.borrow(),
        [(2_000 - MINIMUM_AWAY, 2_000)]
    );
}

#[test]
fn a_long_absence_still_needs_the_minimum_events() {
    let mut recorder = Recorder::new(Some(claim_of(Some(0), vec![entry(1_500)])));
    recorder.entries = vec![entry(1_600)];
    ports(&recorder).run(&returning(vec![leg(true)]), away_policy(), true);
    assert!(
        recorder.publications.borrow().is_empty(),
        "{:?}",
        recorder.steps()
    );
}
