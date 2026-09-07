use super::*;

#[test]
fn every_answered_table_renders_and_parses_back_carrying_its_own_values() {
    let text = render(&every_table_armed()).expect("a fully answered walk renders");
    let config = parse_config(&text).unwrap_or_else(|error| panic!("{error:?}\n{text}"));

    assert_eq!(
        config.plugins["mobile"].settings["token"].as_str(),
        Some("moshi-secret")
    );
    assert_eq!(
        config.plugins["hermes"].settings["key"].as_str(),
        Some("hermes-secret")
    );
    let hue = &config.plugins["hue"].settings;
    assert_eq!(hue["bridge"].as_str(), Some("192.168.1.9"));
    assert_eq!(hue["key"].as_str(), Some("hue-secret"));
    assert_eq!(
        hue["rooms"]
            .as_array()
            .map(|rooms| rooms.iter().filter_map(|room| room.as_str()).collect()),
        Some(vec!["Studio", "Kitchen"])
    );
    let router = &config.plugins["router"].settings;
    assert_eq!(router["type"].as_str(), Some("unifi"));
    assert_eq!(router["router_url"].as_str(), Some("https://192.168.1.1"));
    assert_eq!(router["api_key"].as_str(), Some("router-secret"));
    assert_eq!(router["device_hostname"].as_str(), Some("phone"));
    assert_eq!(config.focus_silence, vec!["Sleep".to_string()]);
    assert_eq!(config.nag_after_secs, 300);
}

#[test]
fn an_empty_walk_still_renders_the_core_at_its_defaults() {
    let text = render(&toml::Table::new()).expect("an empty walk still renders");
    let config = parse_config(&text).unwrap_or_else(|error| panic!("{error:?}\n{text}"));
    assert!(config.plugins["mobile"].enabled);
    assert!(config.plugins["macos-banner"].enabled);
    assert!(config.daemon_enabled);
    assert_eq!(config.recap, crate::config::Recap::default());
    for opt_in in ["hermes", "hue", "router"] {
        assert!(!config.plugins.contains_key(opt_in));
    }
    assert!(config.focus_silence.is_empty());
    assert_eq!(config.nag_after_secs, 0);
    assert!(config.lights.is_none());
}

#[test]
fn an_armed_but_unspecified_lights_table_renders_every_locked_default_uncommented() {
    // ANY LIGHTS KEY AT ALL is the operator asking for the lamps, so
    // every one of the five locked shapes is written live rather than
    // waiting on a value nobody supplied. The assertion is against the
    // code's own `Default`, never against a literal copied out of the
    // layout, so a default that drifts here fails this test rather than
    // shipping quietly.
    let mut values = toml::Table::new();
    values.insert("lights".to_string(), toml::Value::Table(toml::Table::new()));
    let text = render(&values).expect("an armed-empty lights table renders");
    let config = parse_config(&text).unwrap_or_else(|error| panic!("{error:?}\n{text}"));
    let lights = config.lights.expect("lights was armed");
    assert_eq!(*lights, crate::config::Lights::default());
    assert_eq!(
        lights.blocked.give_up_after_secs,
        pns_domain::lamps::config::DEFAULT_BLOCKED_GIVE_UP_AFTER_SECS
    );
}

#[test]
fn recap_defaults_are_asserted_against_the_code_rather_than_copied_literals() {
    let text = render(&toml::Table::new()).expect("an empty walk still renders");
    let config = parse_config(&text).unwrap_or_else(|error| panic!("{error:?}\n{text}"));
    assert_eq!(config.recap, crate::config::Recap::default());
    assert_eq!(
        config.plugins["mobile"].settings["submit_deadline_secs"].as_integer(),
        Some(crate::config::DEFAULT_SUBMIT_DEADLINE_SECS as i64)
    );
}

#[test]
fn core_and_armed_lights_defaults_are_written_live_never_commented() {
    // TEXT-LEVEL, NOT PARSED: a `Default` mistakenly changed to an
    // `Example` still parses back to the same value, since `parse_config`
    // fills in the identical default either way, so a test that only
    // reads the parsed config cannot tell a live default line from a
    // commented one that happens to match. This test reads the rendered
    // TEXT, scoped to each table's own heading so a shared key name
    // (`duration_ms`, `high`, `low`) cannot borrow another table's line.
    let text = render(&toml::Table::new()).expect("an empty walk still renders");
    for expected in [
        "[plugins.mobile]\nenabled = true\n",
        "[plugins.macos-banner]\nenabled = true\n",
        "[daemon]\nenabled = true\n",
    ] {
        assert!(text.contains(expected), "{expected} should be live: {text}");
    }
    for expected in [
        "\nreplay_card = true\n",
        "\ndigest = true\n",
        "\ndigest_as_thread = true\n",
    ] {
        assert!(text.contains(expected), "{expected} should be live: {text}");
    }
    // AND, WHILE LIGHTS IS ABSENT, none of its own defaults leak out live.
    assert!(
        !text.contains("\nduration_ms ="),
        "a lights default rendered live while lights is absent: {text}"
    );

    let mut values = toml::Table::new();
    values.insert("lights".to_string(), toml::Value::Table(toml::Table::new()));
    let armed = render(&values).expect("an armed-empty lights table renders");
    // HEADINGS PLUS THEIR PROSE-FREE KEYS, contiguous lines with nothing
    // between them.
    for expected in [
        "[lights.done]\nduration_ms = 4000\nbrightness = 100\n",
        "[lights.failed]\nduration_ms = 4000\nbrightness = 100\n",
        "[lights.blocked]\nduration_ms = 2000\nhigh = 100\nlow = 30\n",
        "[lights.unread]\nduration_ms = 4000\nhigh = 60\nlow = 10\n",
        "[lights.loop]\nduration_ms = 4000\nhigh = 80\nlow = 10\n",
        "[lights.dim]\nduration_ms = 3000\nhigh = 7\nlow = 1\n",
    ] {
        assert!(
            armed.contains(expected),
            "{expected} should be live once lights is armed: {armed}"
        );
    }
    // THE KEYS THAT CARRY THEIR OWN COMMENT (`after_secs`,
    // `threshold_secs`, `lease_timeout_secs`) sit behind that prose
    // rather than right after the previous key's line, so they are
    // checked on their own.
    for expected in [
        "\nafter_secs = 300\n",
        "\nthreshold_secs = 300\n",
        "\nlease_timeout_secs = 3900\n",
    ] {
        assert!(
            armed.contains(expected),
            "{expected} should be live once lights is armed: {armed}"
        );
    }
}

#[test]
fn a_hostile_entry_name_is_refused_rather_than_closing_the_chezmoi_action() {
    for hostile in ["a\"b", "a\\b", "a}}b", "a\nb"] {
        let error = super::secret_action(secret(hostile, "Password").as_table().unwrap())
            .expect_err(&format!(
                "`{hostile}` can break out of the action and must be refused"
            ));
        assert!(error.contains(hostile), "{error}");
    }
}

/// THE MUTANT THIS PINS: the blank-entry refusal removed, letting
/// `keepassxc ""` (or all-whitespace) reach the shipped template and
/// defer the failure to an apply-time vault lookup nobody is standing in
/// front of.
#[test]
fn a_blank_or_whitespace_only_entry_name_is_refused_rather_than_written() {
    for blank in ["", "   ", "\t"] {
        let error = super::secret_action(secret(blank, "Password").as_table().unwrap())
            .expect_err(&format!("`{blank:?}` names no entry and must be refused"));
        assert!(error.contains("blank"), "{error}");
    }
}

#[test]
fn the_routing_prose_is_always_written_and_the_example_only_when_nothing_is_declared() {
    // A FRESH MACHINE LEARNS THE THREE TARGET KEYS FROM THIS RENDER ALONE:
    // the wizard never asks about the lamp map, so the example is what an
    // operator copies. It is commented whichever way `[lights]` reads, and
    // it steps aside for a real declaration, which is the better example.
    let mut declared_lights = toml::Table::new();
    let mut rooms = toml::Table::new();
    rooms.insert(
        "Kitchen".to_string(),
        toml::Value::Table(toml::Table::new()),
    );
    declared_lights.insert("room".to_string(), toml::Value::Table(rooms));
    let mut declared = toml::Table::new();
    declared.insert("lights".to_string(), toml::Value::Table(declared_lights));

    let mut armed_empty = toml::Table::new();
    armed_empty.insert("lights".to_string(), toml::Value::Table(toml::Table::new()));

    for (values, example_expected) in [
        (toml::Table::new(), true),
        (armed_empty, true),
        (declared, false),
    ] {
        let text = render(&values).expect("every lights shape renders");
        assert!(text.contains("# The routing. `dim_window` is"), "{text}");
        assert_eq!(
            text.contains("# [lights.room.\"Studio\"]\n# shows = "),
            example_expected,
            "{text}"
        );
        // AND THE EXAMPLE IS A LINE THE ROSTER SCAN ACCEPTS, spelled the
        // whitespace-exact way, so the wizard's fence keeps reading it.
        crate::config::documented_keys_the_roster_serves(&text);
        let config = parse_config(&text).unwrap_or_else(|error| panic!("{error:?}\n{text}"));
        assert!(
            config
                .lights
                .is_none_or(|lights| !lights.rooms.contains_key("Studio"))
        );
    }
}
