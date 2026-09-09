use super::*;

// --- [nag] ---------------------------------------------------------------

/// The nag's one key, which is the switch AND the schedule.
///
/// `[focus] silence`'S OWN PRECEDENT: naming nothing and switching off are
/// one statement, so there is no second `enabled` key that can disagree
/// with the first. DEFAULT OFF, unlike `[daemon]` beside it, because this
/// table gates something that INTERRUPTS and because the feature needs a
/// `chezmoi apply` and a running daemon before it works at all.
#[test]
fn the_nag_table_reads_one_schedule_defaults_off_and_zero_is_off_rather_than_an_error() {
    assert_eq!(
        parse_config("").unwrap().nag_after_secs,
        0,
        "no table at all is the feature off"
    );
    assert_eq!(
        parse_config("[nag]\nafter_secs = 300\n")
            .unwrap()
            .nag_after_secs,
        300
    );
    assert_eq!(
        parse_config("[nag]\nafter_secs = 0\n")
            .unwrap()
            .nag_after_secs,
        0,
        "zero is the same statement as no table, and it is not an error"
    );
    // The floor and the ceiling are admitted at their own edges.
    assert_eq!(
        parse_config("[nag]\nafter_secs = 30\n")
            .unwrap()
            .nag_after_secs,
        30
    );
    assert_eq!(
        parse_config("[nag]\nafter_secs = 3600\n")
            .unwrap()
            .nag_after_secs,
        3600
    );
}

/// Every way a schedule can fail to be one, each naming the offender.
///
/// THE FLOOR EXISTS because a nudge arriving before the operator could
/// plausibly have reached their phone is exactly the stacking the design
/// forbids; THE CEILING mirrors `summarizer_deadline_secs` and must sit
/// inside the daemon's own registration window, which it does by three
/// orders of magnitude.
#[test]
fn a_schedule_that_is_not_a_count_of_seconds_is_refused_by_name() {
    for (case, text, named) in [
        ("negative", "[nag]\nafter_secs = -1\n", "after_secs"),
        (
            "a duration string",
            "[nag]\nafter_secs = \"5m\"\n",
            "after_secs",
        ),
        ("fractional", "[nag]\nafter_secs = 300.5\n", "after_secs"),
        ("a list", "[nag]\nafter_secs = [300]\n", "after_secs"),
        ("under the floor", "[nag]\nafter_secs = 29\n", "after_secs"),
        (
            "over the ceiling",
            "[nag]\nafter_secs = 3601\n",
            "after_secs",
        ),
        (
            "a misspelled key",
            "[nag]\nafter_seconds = 300\n",
            "after_seconds",
        ),
        ("a non-table nag", "nag = 300\n", "is not a table"),
    ] {
        match parse_config(text).unwrap_err() {
            ConfigError::Invalid(message) => assert!(
                message.contains("nag") && message.contains(named),
                "{case}: the offender is named: {message}"
            ),
            other => panic!("{case}: expected Invalid, got {other:?}"),
        }
    }
}
