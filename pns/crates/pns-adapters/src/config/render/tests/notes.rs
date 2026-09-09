use super::*;

#[test]
fn a_note_renders_above_its_heading_as_a_commented_line() {
    let mut hermes = toml::Table::new();
    hermes.insert(
        "note".to_string(),
        toml::Value::String("armed for the pns-recap route".to_string()),
    );
    hermes.insert(
        "key".to_string(),
        toml::Value::String("hermes-secret".to_string()),
    );
    let mut plugins = toml::Table::new();
    plugins.insert("hermes".to_string(), toml::Value::Table(hermes));
    let mut values = toml::Table::new();
    values.insert("plugins".to_string(), toml::Value::Table(plugins));

    let text = render(&values).expect("a noted table renders");
    assert!(
        text.contains("# armed for the pns-recap route\n[plugins.hermes]"),
        "{text}"
    );
    // AND `note` NEVER REACHES THE PARSED CONFIG: it is a renderer
    // directive, not a key the roster serves, so stripping it before a
    // round-trip comparison is not a workaround, it never round-trips.
    let config = parse_config(&text).unwrap_or_else(|error| panic!("{error:?}\n{text}"));
    assert!(!config.plugins["hermes"].settings.contains_key("note"));
}

#[test]
fn a_note_holding_a_newline_stays_commented_on_every_line() {
    // THE INJECTION CASE. A note that could open a live heading or an
    // uncommented key on its second line would let a values file smuggle
    // arbitrary config text past every other refusal in this module.
    let mut hermes = toml::Table::new();
    hermes.insert(
        "note".to_string(),
        toml::Value::String(
            "line one\n[plugins.hue]\nenabled = true\nbridge = \"hostile\"".to_string(),
        ),
    );
    hermes.insert(
        "key".to_string(),
        toml::Value::String("hermes-secret".to_string()),
    );
    let mut plugins = toml::Table::new();
    plugins.insert("hermes".to_string(), toml::Value::Table(hermes));
    let mut values = toml::Table::new();
    values.insert("plugins".to_string(), toml::Value::Table(plugins));

    let text = render(&values).expect("a multi-line note renders");
    for line in text.lines() {
        if line.contains("hostile") {
            assert!(
                line.starts_with('#'),
                "an injected line escaped its comment: {line}"
            );
        }
    }
    let config = parse_config(&text).unwrap_or_else(|error| panic!("{error:?}\n{text}"));
    // AND THE INJECTED TABLE NEVER ARRIVED: a real `[plugins.hue]` armed
    // by the note would be the exact failure this test exists to catch.
    assert!(!config.plugins.contains_key("hue"));
}

#[test]
fn declarations_at_every_level_render_sorted_hostile_names_quoted_with_their_own_notes() {
    // ONE SAFE ROOM IS NOT ENOUGH: lamp and zone rendering, the sort
    // order, a hostile name and a target's own note all need their own
    // exercise, or removing any of them survives every existing test.
    let mut lamps = toml::Table::new();
    lamps.insert(
        "Zeta Lamp".to_string(),
        toml::Value::Table(toml::Table::new()),
    );
    lamps.insert(
        "Alpha Lamp".to_string(),
        toml::Value::Table(toml::Table::new()),
    );

    let mut hostile_target = toml::Table::new();
    hostile_target.insert(
        "note".to_string(),
        toml::Value::String("the desk lamp".to_string()),
    );
    let mut rooms = toml::Table::new();
    rooms.insert(
        "Zeta Room".to_string(),
        toml::Value::Table(toml::Table::new()),
    );
    rooms.insert(
        "Alpha \"Room\"".to_string(),
        toml::Value::Table(hostile_target),
    );

    let mut zones = toml::Table::new();
    zones.insert(
        "Zeta Zone".to_string(),
        toml::Value::Table(toml::Table::new()),
    );
    zones.insert(
        "Alpha Zone".to_string(),
        toml::Value::Table(toml::Table::new()),
    );

    let mut lights = toml::Table::new();
    lights.insert("lamp".to_string(), toml::Value::Table(lamps));
    lights.insert("room".to_string(), toml::Value::Table(rooms));
    lights.insert("zone".to_string(), toml::Value::Table(zones));
    let mut values = toml::Table::new();
    values.insert("lights".to_string(), toml::Value::Table(lights));

    let text = render(&values).expect("declarations at every level render");

    // SORTED, ALPHA BEFORE ZETA, at each of the three levels: `toml::Table`
    // is BTreeMap-ordered, so this catches sorting being disabled.
    for level in ["lamp", "room", "zone"] {
        let alpha = text
            .find(&format!("[lights.{level}.\"Alpha"))
            .unwrap_or_else(|| panic!("no Alpha declaration at `{level}`: {text}"));
        let zeta = text
            .find(&format!("[lights.{level}.\"Zeta"))
            .unwrap_or_else(|| panic!("no Zeta declaration at `{level}`: {text}"));
        assert!(
            alpha < zeta,
            "`{level}` did not sort Alpha before Zeta: {text}"
        );
    }

    // A QUOTE IN THE NAME IS ESCAPED, never raw-interpolated into the
    // heading, which would otherwise close the TOML key string early.
    assert!(
        text.contains("[lights.room.\"Alpha \\\"Room\\\"\"]"),
        "{text}"
    );
    // AND ITS OWN NOTE renders above its own heading, not the room's name
    // it happens to share no other target with.
    assert!(
        text.contains("# the desk lamp\n[lights.room.\"Alpha \\\"Room\\\"\"]"),
        "{text}"
    );

    let config = parse_config(&text).unwrap_or_else(|error| panic!("{error:?}\n{text}"));
    let lights = config.lights.expect("lights was armed");
    assert!(lights.lamps.contains_key("Alpha Lamp"));
    assert!(lights.lamps.contains_key("Zeta Lamp"));
    assert!(lights.zones.contains_key("Alpha Zone"));
    assert!(lights.zones.contains_key("Zeta Zone"));
}

#[test]
fn a_note_holding_a_chezmoi_action_opening_is_refused_by_name() {
    // A NOTE IS WRITTEN AS A RAW COMMENT, never a quoted string, so `quoted`'s
    // brace-splitting cannot protect it: chezmoi's template engine reads
    // `{{ ... }}` inside a comment exactly like anywhere else in the file, so
    // the only safe answer is refusing the note outright.
    let mut hermes = toml::Table::new();
    hermes.insert(
        "note".to_string(),
        toml::Value::String("safe {{ printf \"pwned\" }} unsafe".to_string()),
    );
    let mut plugins = toml::Table::new();
    plugins.insert("hermes".to_string(), toml::Value::Table(hermes));
    let mut values = toml::Table::new();
    values.insert("plugins".to_string(), toml::Value::Table(plugins));

    let error = render(&values).expect_err("a note opening a chezmoi action must be refused");
    assert!(error.contains("note"), "{error}");
}

#[test]
fn a_note_holding_a_forbidden_control_character_is_refused_by_name() {
    // `write_note` PREFIXES EACH `\n`-SPLIT LINE WITH `# `, so a NUL or
    // DEL sitting mid-line, or a lone CR that split() never breaks on,
    // rides straight into the comment and makes `parse_config` refuse the
    // text `render` just claimed to succeed on. CRLF is normalized first,
    // since it is an ordinary line ending rather than a hostile control.
    for hostile in [
        "line one\r\nline two",
        "bad\u{0}byte",
        "bad\u{7f}byte",
        "lone\rcarriage",
    ] {
        let mut hermes = toml::Table::new();
        hermes.insert("note".to_string(), toml::Value::String(hostile.to_string()));
        hermes.insert(
            "key".to_string(),
            toml::Value::String("hermes-secret".to_string()),
        );
        let mut plugins = toml::Table::new();
        plugins.insert("hermes".to_string(), toml::Value::Table(hermes));
        let mut values = toml::Table::new();
        values.insert("plugins".to_string(), toml::Value::Table(plugins));

        let result = render(&values);
        if hostile.contains("\r\n") {
            // CRLF is the one shape that must be ACCEPTED, normalized to a
            // plain newline rather than refused as a control character.
            let text = result.expect("CRLF normalizes rather than refusing");
            assert!(text.contains("# line one\n# line two\n"), "{text}");
            let config = parse_config(&text).unwrap_or_else(|error| panic!("{error:?}\n{text}"));
            assert!(config.plugins.contains_key("hermes"));
        } else {
            let error = result.expect_err(&format!(
                "{hostile:?} must be refused rather than rendered into an unparsable comment"
            ));
            assert!(error.contains("note"), "{error}");
        }
    }
}

#[test]
fn the_recap_prose_keeps_the_hook_path_and_note_limit_facts_the_template_carries() {
    // X2's template-prose rule: the shipped template's facts win except
    // where they name the operator's own environment. Dropping the hook
    // PATH explanation on `repos` or the twenty-five note limit on
    // `review_notes` loses a real fact nothing else states.
    let text = render(&toml::Table::new()).expect("an empty walk still renders");
    assert!(text.contains("FOUND ON PATH"), "{text}");
    assert!(text.contains("Twenty-five notes"), "{text}");
}

#[test]
fn the_header_scopes_the_credential_arming_claim_to_the_plugins_it_names() {
    // Focus, the nag and the lamp map are opt-in tables that need no
    // credential at all; only three of the plugins do (hue, hermes,
    // router), so a blanket "everything else is armed with a
    // credential" misstates all three of them.
    let text = render(&toml::Table::new()).expect("an empty walk still renders");
    assert!(
        !text.contains("Everything else is armed with a credential"),
        "{text}"
    );
    assert!(text.contains("need no credential at all"), "{text}");
}

#[test]
fn a_note_above_the_bare_lights_heading_renders_like_any_other_tables() {
    // `lights` IS TAKEN APART BEFORE IT IS WRITTEN, so its own `note` has
    // to be pulled out with `refresh_secs` or the leftover check refuses
    // it as an unknown key, the one table a values file could not comment.
    let mut lights = toml::Table::new();
    lights.insert(
        "note".to_string(),
        toml::Value::String("the lamps this machine drives".to_string()),
    );
    let mut values = toml::Table::new();
    values.insert("lights".to_string(), toml::Value::Table(lights));

    let text = render(&values).expect("a noted lights table renders");
    assert!(
        text.contains("# the lamps this machine drives\n[lights]\n"),
        "{text}"
    );
    let config = parse_config(&text).unwrap_or_else(|error| panic!("{error:?}\n{text}"));
    assert_eq!(
        *config.lights.expect("lights was armed"),
        crate::config::Lights::default()
    );
}
