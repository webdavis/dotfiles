use super::*;

/// The first failure is the news that something broke, and for a permanent
/// refusal it is the only moment there will ever be.
#[test]
fn the_first_failure_is_announced() {
    assert!(warrants_notification(0, false));
}

/// The attempts between the first failure and the dead-letter say nothing the
/// first one did not, and a banner per attempt is what teaches an operator to
/// dismiss the banner that matters.
#[test]
fn the_attempts_in_between_stay_quiet() {
    for retries in 1..20 {
        assert!(
            !warrants_notification(retries, false),
            "retry {retries} spoke"
        );
    }
}

/// Giving up is news the first failure did not carry: the page is lost rather
/// than late.
#[test]
fn the_dead_letter_is_announced_however_many_attempts_it_took() {
    assert!(warrants_notification(19, true));
}
