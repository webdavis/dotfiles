use super::*;

#[test]
fn a_phone_marker_override_survives_rendering_and_parsing() {
    let values = "[plugins.phone]\nmarker_file = '~/custom/attention'"
        .parse()
        .unwrap();
    let text = render(&values).expect("phone override renders");
    assert_eq!(
        parse_config(&text).unwrap().phone_marker_file.as_deref(),
        Some("~/custom/attention")
    );
}

#[test]
fn every_answered_table_renders_and_parses_back_carrying_its_own_values() {
    let text = render(&every_table_armed()).expect("a fully answered walk renders");
    let config = parse_config(&text).unwrap_or_else(|error| panic!("{error:?}\n{text}"));

    assert_eq!(
        config.plugins["phone"].settings["device_token"].as_str(),
        Some("moshi-secret")
    );
    assert_eq!(
        config.plugins["hermes"].settings["keys"]["pns-events"].as_str(),
        Some("hermes-secret")
    );
    let hue = &config.plugins["lights"].settings;
    assert_eq!(hue["bridge_host"].as_str(), Some("192.168.1.9"));
    assert_eq!(hue["api_key"].as_str(), Some("hue-secret"));
    let router = &config.plugins["home_presence"].settings;
    assert_eq!(router["type"].as_str(), Some("unifi"));
    assert_eq!(router["url"].as_str(), Some("https://192.168.1.1"));
    assert_eq!(router["api_key"].as_str(), Some("router-secret"));
    assert_eq!(router["device_hostname"].as_str(), Some("phone"));
    assert_eq!(config.focus_modes, vec!["Sleep".to_string()]);
    assert_eq!(config.remind_delay_secs, 300);
    assert_eq!(
        config.stale_escalate_after_secs, 3600,
        "a defaulted key ships uncommented at its default (operator ruling, 2026-08-31)"
    );
}

#[test]
fn an_empty_walk_still_renders_the_core_at_its_defaults() {
    let text = render(&toml::Table::new()).expect("an empty walk still renders");
    let config = parse_config(&text).unwrap_or_else(|error| panic!("{error:?}\n{text}"));
    // EVERY SWITCH AT ITS OWN DEFAULT, which is what a walk that said
    // nothing asked for: the core plugin tables are written, and written
    // off, because `[plugins.*] enabled` defaults false.
    assert!(!config.plugins["phone"].enabled);
    assert!(!config.plugins["banner"].enabled);
    assert!(config.daemon_enabled);
    assert!(config.focus_enabled);
    assert!(config.stale_enabled);
    assert_eq!(config.recap, crate::config::Recap::default());
    for opt_in in ["hermes", "lights", "home_presence"] {
        assert!(!config.plugins.contains_key(opt_in));
    }
    assert!(config.focus_modes.is_empty());
    assert_eq!(config.remind_delay_secs, 0);
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
        lights.blocked.lease_expiry_secs,
        pns_domain::lamps::config::DEFAULT_BLOCKED_LEASE_EXPIRY_SECS
    );
}

#[test]
fn recap_defaults_are_asserted_against_the_code_rather_than_copied_literals() {
    let text = render(&toml::Table::new()).expect("an empty walk still renders");
    let config = parse_config(&text).unwrap_or_else(|error| panic!("{error:?}\n{text}"));
    assert_eq!(config.recap, crate::config::Recap::default());
    assert_eq!(
        crate::config::ack_deadline(&config).unwrap(),
        crate::config::DEFAULT_ACK_DEADLINE
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
    // (`duration`, `high_percent`, `low_percent`) cannot borrow another
    // table's line.
    let text = render(&toml::Table::new()).expect("an empty walk still renders");
    for expected in [
        "[plugins.phone]\nenabled = false\n",
        "[plugins.banner]\nenabled = false\n",
        "[daemon]\nenabled = true\n",
        "[stale]\nenabled = true\n",
    ] {
        assert!(text.contains(expected), "{expected} should be live: {text}");
    }
    for expected in ["\nreplay_card = true\n", "\npost_window_recap = true\n"] {
        assert!(text.contains(expected), "{expected} should be live: {text}");
    }
    // AND, WHILE LIGHTS IS ABSENT, none of its own defaults leak out live.
    assert!(
        !text.contains("\nduration ="),
        "a lights default rendered live while lights is absent: {text}"
    );

    let mut values = toml::Table::new();
    values.insert("lights".to_string(), toml::Value::Table(toml::Table::new()));
    let armed = render(&values).expect("an armed-empty lights table renders");
    // HEADINGS PLUS THEIR PROSE-FREE KEYS, contiguous lines with nothing
    // between them.
    for expected in [
        "[lights.done]\nduration = \"4s\"\nbrightness_percent = 100\n",
        "[lights.failed]\nduration = \"4s\"\nbrightness_percent = 100\n",
        "[lights.blocked]\nduration = \"2s\"\nhigh_percent = 100\nlow_percent = 30\n",
        "[lights.unseen]\nduration = \"4s\"\nhigh_percent = 60\nlow_percent = 10\n",
        "[lights.loop]\nduration = \"4s\"\nhigh_percent = 80\nlow_percent = 10\n",
        "[lights.dim]\nduration = \"3s\"\nhigh_percent = 7\nlow_percent = 1\n",
    ] {
        assert!(
            armed.contains(expected),
            "{expected} should be live once lights is armed: {armed}"
        );
    }
    // THE KEYS THAT CARRY THEIR OWN COMMENT (`arm_after` on both tables,
    // and `lease_expiry`) sit behind that prose rather than right after
    // the previous key's line, so they are checked on their own.
    for expected in ["\narm_after = \"5m\"\n", "\nlease_expiry = \"65m\"\n"] {
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
            text.contains("# [lights.room.\"Studio\"]\n# behaviours = "),
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

#[test]
fn the_phone_marker_note_points_at_the_tap_install_subcommand() {
    let text = render(&"".parse().unwrap()).expect("the shipped posture renders");
    assert!(text.contains("Setup guide: pns tap install."), "{text}");
    assert!(!text.contains("pns tap --install"), "{text}");
}
