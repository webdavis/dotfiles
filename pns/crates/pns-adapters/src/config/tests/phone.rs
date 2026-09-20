use super::*;

// --- the moshi acknowledgement deadline -----------------------------------

#[test]
fn the_acknowledgement_deadline_is_a_duration_defaulted_to_five_seconds() {
    // FIVE SECONDS, the crate's own house number for a local pipe that
    // should have been instant: it is `PNS_PAYLOAD_DEADLINE`'s default,
    // bounding the same kind of thing on the same hook. The submission is
    // a registration with a daemon, measured at roughly a tenth of a
    // second, so five is about thirty times the observed round trip.
    assert_eq!(
        ack_deadline(&parse_config("").unwrap()).unwrap(),
        Duration::from_secs(5),
        "no config at all is still bounded"
    );
    assert_eq!(
        ack_deadline(&parse_config("[plugins.phone]\nenabled = true\ntype = \"moshi\"\n").unwrap())
            .unwrap(),
        Duration::from_secs(5),
        "a phone table that does not state one is the default"
    );
    // THE TABLE IS ARMED IN EVERY CASE BELOW, switch and backend both,
    // because that is what `armed_phone` reads and this key is read
    // through it: a duration under a table nobody switched on, or under
    // one naming a backend nothing implements, is not a bound this binary
    // owns.
    assert_eq!(
        ack_deadline(
            &parse_config(
                "[plugins.phone]\nenabled = true\ntype = \"moshi\"\n\
                     ack_deadline = \"30s\"\n"
            )
            .unwrap()
        )
        .unwrap(),
        Duration::from_secs(30),
        "the operator's own duration is the bound"
    );
    // OFF THE PHONE TABLE. Every plugin's settings reach this layer in
    // the same shape, so a reader spelled against the wrong table would
    // take a duration the operator wrote for something else, or miss the
    // one they wrote for this. The misplacement is refused a whole layer
    // earlier, at the load that judges each table's own vocabulary, so
    // this states both halves: the key on another table never parses, and
    // a config carrying no phone table at all is still the default.
    assert!(
        parse_config("[plugins.lights]\nack_deadline = \"30s\"\n").is_err(),
        "the phone table's key is not part of the lights vocabulary"
    );
    assert_eq!(
        ack_deadline(&parse_config("[plugins.lights]\nenabled = true\n").unwrap()).unwrap(),
        Duration::from_secs(5),
        "another plugin's table is not where the phone bound is read"
    );
}

#[test]
fn a_phone_table_naming_no_backend_contributes_no_settings_at_all() {
    // THE DEADLINE INCLUDED, which is the half a reader spelled against
    // the table directly used to miss. `type` says which backend every
    // setting under the table belongs to, so a table naming one nothing
    // implements has no settings this binary may read as moshi's: an
    // `ack_deadline = "1s"` written for some other backend must not
    // shorten the window pns waits for a moshi submission in.
    let refused = parse_config(
        "[plugins.phone]\nenabled = true\ntype = \"pushover\"\nack_deadline = \"1s\"\n",
    )
    .unwrap();
    match ack_deadline(&refused) {
        Err(ConfigError::Invalid(message)) => {
            assert!(message.contains("\"pushover\""), "quoting it: {message}");
            assert!(message.contains("type"), "and naming the key: {message}");
        }
        other => panic!("expected the type refusal, got {other:?}"),
    }
    // THE POSITIVE CONTROL: a refusal that fired on every table would pass
    // the assertion above and take every operator's deadline away.
    let armed =
        parse_config("[plugins.phone]\nenabled = true\ntype = \"moshi\"\nack_deadline = \"30s\"\n")
            .unwrap();
    assert_eq!(ack_deadline(&armed).unwrap(), Duration::from_secs(30));
    // AND A TABLE THE OPERATOR SWITCHED OFF IS INERT, its settings with
    // it: one switch, one answer, rather than a per-key exception nobody
    // could predict from the flag they set.
    let off = parse_config(
        "[plugins.phone]\nenabled = false\ntype = \"moshi\"\nack_deadline = \"30s\"\n",
    )
    .unwrap();
    assert_eq!(ack_deadline(&off).unwrap(), Duration::from_secs(5));
}

#[test]
fn an_acknowledgement_deadline_outside_the_range_is_refused_by_name() {
    // REFUSED IN BOTH DIRECTIONS, and each refusal names the key, because
    // "config invalid" without a noun is a hunt.
    //
    // ZERO IS A TRAP HERE, unlike `summarizer_deadline`'s zero. A
    // deadline that fires before the daemon can possibly answer is this
    // feature switched off by accident: every approval would lose its
    // phone card while the operator believed they had merely tightened a
    // bound. The ceiling mirrors `summarizer_deadline`'s hour rather than
    // the harness's own ten minutes, because another tool's number is not
    // ours to hard-code and Codex's differs.
    for stated in ["\"0s\"", "\"500ms\"", "\"2h\"", "5", "9.5", "[5]", "\"\""] {
        let config = parse_config(&format!(
            "[plugins.phone]\nenabled = true\ntype = \"moshi\"\n\
                 ack_deadline = {stated}\n"
        ))
        .unwrap();
        match ack_deadline(&config) {
            Err(ConfigError::Invalid(message)) => assert!(
                message.contains("ack_deadline"),
                "the offender is named for {stated}: {message}"
            ),
            other => panic!("expected a named refusal for {stated}, got {other:?}"),
        }
    }
}

// --- the attention marker --------------------------------------------------

#[test]
fn marker_paths_are_explicit_and_values_never_leak_in_refusals() {
    for path in [
        "",
        "relative-secret",
        "~other/private",
        "/secret\npath",
        "/secret\0path",
    ] {
        let config = format!(
            "[plugins.phone]\nmarker_file = {}",
            serde_json::to_string(path).unwrap()
        );
        let error = parse_config(&config).unwrap_err();
        assert!(
            error.detail().contains("plugins.phone"),
            "names the table: {error:?}"
        );
        assert!(error.detail().contains("marker_file"), "{error:?}");
        assert!(!error.detail().contains("secret"));
    }
    // READ WITH THE TABLE SWITCHED OFF TOO, because `pns tap` writes the
    // marker whether or not the card is armed.
    for config in [
        "[plugins.phone]\nmarker_file = '/absolute/path'",
        "[plugins.phone]\nenabled = true\ntype = \"moshi\"\nmarker_file = '/absolute/path'",
    ] {
        assert_eq!(
            parse_config(config).unwrap().phone_marker_file.as_deref(),
            Some("/absolute/path"),
            "case: {config}"
        );
    }
    assert!(parse_config("[plugins.phone]\nunknown = 'private'").is_err());
    assert!(
        parse_config("[plugins.phone]")
            .unwrap()
            .phone_marker_file
            .is_none()
    );
}

// --- the headings this table replaced --------------------------------------

#[test]
fn the_old_top_level_phone_table_is_refused_naming_the_plugin_heading() {
    // THE MUTANT THIS PINS: `[phone]` quietly admitted again, which is a
    // marker path pns never reads and a tap that lands somewhere else.
    let Err(ConfigError::Invalid(said)) = parse_config("[phone]\nmarker_file = '/absolute/path'\n")
    else {
        panic!("`[phone]` was accepted");
    };
    assert!(said.contains("[plugins.phone]"), "{said}");
    assert!(said.contains("marker_file"), "{said}");
}
