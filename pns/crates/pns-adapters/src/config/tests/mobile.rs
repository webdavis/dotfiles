use super::*;

// --- the mobile submission deadline --------------------------------------

#[test]
fn the_mobile_submission_deadline_is_a_count_of_seconds_defaulted_to_five() {
    // FIVE SECONDS, the crate's own house number for a local pipe that
    // should have been instant: it is `PNS_PAYLOAD_DEADLINE_MS`'s default,
    // bounding the same kind of thing on the same hook. The submission is
    // a registration with a daemon, measured at roughly a tenth of a
    // second, so five is about thirty times the observed round trip.
    assert_eq!(
        submit_deadline(&parse_config("").unwrap()).unwrap(),
        Duration::from_secs(5),
        "no config at all is still bounded"
    );
    assert_eq!(
        submit_deadline(
            &parse_config("[plugins.mobile]\nenabled = true\ntype = \"moshi\"\n").unwrap()
        )
        .unwrap(),
        Duration::from_secs(5),
        "a mobile table that does not state one is the default"
    );
    // THE TABLE IS ARMED IN EVERY CASE BELOW, switch and backend both,
    // because that is what `armed_mobile` reads and this key is read
    // through it: a number under a table nobody switched on, or under one
    // naming a backend nothing implements, is not a bound this binary owns.
    assert_eq!(
        submit_deadline(
            &parse_config(
                "[plugins.mobile]\nenabled = true\ntype = \"moshi\"\n\
                     submit_deadline_secs = 30\n"
            )
            .unwrap()
        )
        .unwrap(),
        Duration::from_secs(30),
        "the operator's own number is the bound"
    );
    // OFF THE MOBILE TABLE. Every plugin's settings reach this layer in
    // the same shape, so a reader spelled against the wrong table would
    // take a number the operator wrote for something else, or miss the one
    // they wrote for this. The misplacement is now refused a whole layer
    // earlier, at the load that judges each table's own vocabulary, so
    // this states both halves: the key on another table never parses, and
    // a config carrying no mobile table at all is still the default.
    assert!(
        parse_config("[plugins.hue]\nsubmit_deadline_secs = 30\n").is_err(),
        "the mobile table's key is not part of hue's vocabulary"
    );
    assert_eq!(
        submit_deadline(&parse_config("[plugins.hue]\nenabled = true\n").unwrap()).unwrap(),
        Duration::from_secs(5),
        "another plugin's table is not where the mobile bound is read"
    );
}

#[test]
fn a_mobile_table_naming_no_backend_contributes_no_settings_at_all() {
    // THE DEADLINE INCLUDED, which is the half a reader spelled against
    // the table directly used to miss. `type` says which backend every
    // setting under the table belongs to, so a table naming one nothing
    // implements has no settings this binary may read as moshi's: a
    // `submit_deadline_secs = 1` written for some other backend must not
    // shorten the window pns waits for a moshi submission in.
    let refused = parse_config(
        "[plugins.mobile]\nenabled = true\ntype = \"pushover\"\nsubmit_deadline_secs = 1\n",
    )
    .unwrap();
    match submit_deadline(&refused) {
        Err(ConfigError::Invalid(message)) => {
            assert!(message.contains("\"pushover\""), "quoting it: {message}");
            assert!(message.contains("type"), "and naming the key: {message}");
        }
        other => panic!("expected the type refusal, got {other:?}"),
    }
    // THE POSITIVE CONTROL: a refusal that fired on every table would pass
    // the assertion above and take every operator's deadline away.
    let armed = parse_config(
        "[plugins.mobile]\nenabled = true\ntype = \"moshi\"\nsubmit_deadline_secs = 30\n",
    )
    .unwrap();
    assert_eq!(submit_deadline(&armed).unwrap(), Duration::from_secs(30));
    // AND A TABLE THE OPERATOR SWITCHED OFF IS INERT, its settings with
    // it: one switch, one answer, rather than a per-key exception nobody
    // could predict from the flag they set.
    let off = parse_config(
        "[plugins.mobile]\nenabled = false\ntype = \"moshi\"\nsubmit_deadline_secs = 30\n",
    )
    .unwrap();
    assert_eq!(submit_deadline(&off).unwrap(), Duration::from_secs(5));
}

#[test]
fn a_submission_deadline_that_is_not_a_count_of_seconds_is_refused_by_name() {
    // REFUSED IN BOTH DIRECTIONS, and each refusal names the key, because
    // "config invalid" without a noun is a hunt.
    //
    // ZERO IS A TRAP HERE, unlike `summarizer_deadline_secs`'s zero. A
    // deadline that fires before the daemon can possibly answer is this
    // feature switched off by accident: every approval would lose its
    // phone card while the operator believed they had merely tightened a
    // bound. The ceiling mirrors `MAX_SUMMARIZER_DEADLINE_SECS` rather
    // than the harness's own ten minutes, because another tool's number is
    // not ours to hard-code and Codex's differs.
    for stated in [
        "0",
        "-1",
        "\"5s\"",
        "9.5",
        "[5]",
        "3601",
        "9223372036854775807",
    ] {
        let config = parse_config(&format!(
            "[plugins.mobile]\nenabled = true\ntype = \"moshi\"\n\
                 submit_deadline_secs = {stated}\n"
        ))
        .unwrap();
        match submit_deadline(&config) {
            Err(ConfigError::Invalid(message)) => assert!(
                message.contains("submit_deadline_secs"),
                "the offender is named for {stated}: {message}"
            ),
            other => panic!("expected a named refusal for {stated}, got {other:?}"),
        }
    }
}
