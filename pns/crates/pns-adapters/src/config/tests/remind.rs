use super::*;

// --- [remind] ---------------------------------------------------------------

/// The reminder's one key, which is the switch AND the schedule.
///
/// `[focus] silence`'S OWN PRECEDENT: naming nothing and switching off are
/// one statement, so there is no second `enabled` key that can disagree
/// with the first. DEFAULT OFF, unlike `[daemon]` beside it, because this
/// table gates something that INTERRUPTS and because the feature needs a
/// `chezmoi apply` and a running daemon before it works at all.
#[test]
fn the_remind_table_reads_one_delay_defaults_off_and_zero_is_off_rather_than_an_error() {
    assert_eq!(
        parse_config("").unwrap().remind_delay_secs,
        0,
        "no table at all is the feature off"
    );
    assert_eq!(
        parse_config("[remind]\ndelay = \"5m\"\n")
            .unwrap()
            .remind_delay_secs,
        300
    );
    assert_eq!(
        parse_config("[remind]\ndelay = \"0s\"\n")
            .unwrap()
            .remind_delay_secs,
        0,
        "zero is the same statement as no table, and it is not an error"
    );
    // The floor and the ceiling are admitted at their own edges.
    assert_eq!(
        parse_config("[remind]\ndelay = \"30s\"\n")
            .unwrap()
            .remind_delay_secs,
        30
    );
    assert_eq!(
        parse_config("[remind]\ndelay = \"1h\"\n")
            .unwrap()
            .remind_delay_secs,
        3600
    );
}

/// Every way a delay can fail to be one, each naming the offender.
///
/// THE FLOOR EXISTS because a nudge arriving before the operator could
/// plausibly have reached their phone is exactly the stacking the design
/// forbids; THE CEILING mirrors `summarizer_deadline` and must sit
/// inside the daemon's own registration window, which it does by three
/// orders of magnitude.
#[test]
fn a_delay_that_is_not_a_duration_is_refused_by_name() {
    for (case, text, named) in [
        ("a bare number", "[remind]\ndelay = 300\n", "delay"),
        ("negative", "[remind]\ndelay = \"-1m\"\n", "delay"),
        ("no unit", "[remind]\ndelay = \"300\"\n", "delay"),
        ("a list", "[remind]\ndelay = [\"5m\"]\n", "delay"),
        ("under the floor", "[remind]\ndelay = \"29s\"\n", "delay"),
        ("over the ceiling", "[remind]\ndelay = \"61m\"\n", "delay"),
        (
            "a misspelled key",
            "[remind]\ndelay_secs = \"5m\"\n",
            "delay_secs",
        ),
        ("a non-table remind", "remind = 300\n", "is not a table"),
    ] {
        match parse_config(text).unwrap_err() {
            ConfigError::Invalid(message) => assert!(
                message.contains("remind") && message.contains(named),
                "{case}: the offender is named: {message}"
            ),
            other => panic!("{case}: expected Invalid, got {other:?}"),
        }
    }
}
