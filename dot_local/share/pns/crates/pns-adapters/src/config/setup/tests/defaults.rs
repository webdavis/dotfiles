use super::*;
#[test]
fn a_walk_that_armed_nothing_still_writes_the_core() {
    // THE SHIPPED POSTURE. Declining every question is the common first
    // run, and what it has to leave behind is a machine that banners and
    // cards: an absent `enabled` reads FALSE, so a written config that
    // states neither would take the core away from the machine that just
    // asked for a config.
    let text = compose_config(&Answers::default());
    let config = parsed(&text);
    assert!(config.plugins["macos-banner"].enabled);
    assert!(config.plugins["mobile"].enabled);
    assert_eq!(
        config.plugins["mobile"].settings["type"].as_str(),
        Some("moshi")
    );
    for opt_in in ["hermes", "hue", "router"] {
        assert!(
            !config.plugins.contains_key(opt_in),
            "`{opt_in}` was armed by nobody"
        );
    }
    assert!(config.lights.is_none());
    assert!(config.focus_silence.is_empty());
    assert_eq!(config.nag_after_secs, 0);
    // AND A DECLINED TABLE IS COMMENTED OUT rather than written with empty
    // values, which is the same rule stated about the text rather than
    // about what it parses to: `silence = []` and `rooms = []` load to the
    // same nothing an absent table does, and read as a feature set up.
    for declined in DECLINABLE_TABLES {
        assert!(
            !text.contains(&format!("\n{declined}")),
            "`{declined}` stands uncommented in a walk that armed nothing:\n{text}"
        );
    }
}

#[test]
fn the_values_it_writes_unprompted_are_the_ones_the_code_defaults_to() {
    // WRITTEN OUT AT THEIR DEFAULTS, and the assertion is against the
    // code's own default rather than against the same literals the
    // composer holds: a default moved in `config` and left standing here
    // would otherwise ship a wizard writing yesterday's number as though
    // it were today's.
    let config = parsed(&compose_config(&Answers::default()));
    assert_eq!(config.recap, Recap::default());
    assert!(config.daemon_enabled);
    let mobile = &config.plugins["mobile"].settings;
    assert_eq!(mobile["mobile_watch_card"].as_bool(), Some(false));
    assert_eq!(
        mobile["submit_deadline_secs"].as_integer(),
        Some(DEFAULT_SUBMIT_DEADLINE_SECS as i64)
    );
}

#[test]
fn a_skipped_token_is_commented_out_rather_than_written_empty() {
    // MOBILE STAYS ON EITHER WAY: pairing is what completes it, and a
    // `token = ""` would read as configured while carding nothing.
    let text = compose_config(&Answers::default());
    assert!(text.contains("# token = \"\""), "{text}");
    let config = parsed(&text);
    assert!(config.plugins["mobile"].enabled);
    assert!(!config.plugins["mobile"].settings.contains_key("token"));
}
