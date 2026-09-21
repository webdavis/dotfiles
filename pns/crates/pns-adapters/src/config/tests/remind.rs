use super::*;

// --- [remind] ---------------------------------------------------------------

/// The reminder's one key, whose ABSENCE is the off statement.
///
/// NO KEY DOUBLES AS ITS OWN SWITCH, so `"0s"` is refused by name rather
/// than read as off: the key not being there already says it. DEFAULT OFF,
/// unlike `[daemon]` beside it, because this table gates something that
/// INTERRUPTS and because the feature needs a `chezmoi apply` and a running
/// daemon before it works at all.
#[test]
fn the_remind_table_reads_one_delay_and_defaults_off() {
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

/// ZERO IS REFUSED BY NAME, and the refusal says where off lives instead.
#[test]
fn a_zero_delay_is_refused_and_points_at_the_absent_key() {
    let said = refusal("[remind]\ndelay = \"0s\"\n");
    assert!(
        said.contains("remind") && said.contains("delay") && said.contains("unset"),
        "the refusal names the table, the key and the off statement: {said}"
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
