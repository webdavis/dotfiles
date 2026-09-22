use super::*;

fn words(argv: &[&str]) -> Vec<String> {
    argv.iter().map(|word| word.to_string()).collect()
}

#[test]
fn no_bound_is_an_override_that_stands_until_cleared() {
    assert_eq!(parse_bound(&words(&[]), Some(1_000), Some(600)), Ok(None));
}

#[test]
fn a_duration_bound_is_added_to_the_clock() {
    assert_eq!(
        parse_bound(&words(&["--for", "2h"]), Some(1_000), Some(600)),
        Ok(Some(1_000 + 2 * 3_600))
    );
}

#[test]
fn a_clock_time_that_has_passed_today_means_tomorrow() {
    // 10:00 local, asked to run until 09:00: that is nine tomorrow morning.
    let until = parse_bound(&words(&["--until", "09:00"]), Some(10_000), Some(10 * 60))
        .expect("a bound")
        .expect("a time");
    assert_eq!(until, 10_000 + 23 * 3_600, "twenty three hours from now");
    let later = parse_bound(&words(&["--until", "17:30"]), Some(10_000), Some(10 * 60))
        .expect("a bound")
        .expect("a time");
    assert_eq!(later, 10_000 + 7 * 3_600 + 30 * 60);
}

#[test]
fn a_bad_bound_is_a_usage_error() {
    assert!(matches!(
        parse_bound(&words(&["--for", "900h"]), Some(0), Some(0)),
        Err(BoundError::Usage(_))
    ));
    assert!(matches!(
        parse_bound(&words(&["--until", "25:00"]), Some(0), Some(0)),
        Err(BoundError::Usage(_))
    ));
    assert!(matches!(
        parse_bound(&words(&["--for"]), Some(0), Some(0)),
        Err(BoundError::Usage(_))
    ));
    assert!(matches!(
        parse_bound(&words(&["--soon", "2h"]), Some(0), Some(0)),
        Err(BoundError::Usage(_))
    ));
}

#[test]
fn a_bound_that_needs_a_clock_is_a_clock_error_not_a_usage_error() {
    // A CLOCK ERROR IS EXIT 1, a state error like every other one in this
    // command, never exit 2's usage refusal for a typo the operator did not
    // make.
    assert!(matches!(
        parse_bound(&words(&["--for", "2h"]), None, None),
        Err(BoundError::Clock(_))
    ));
}
