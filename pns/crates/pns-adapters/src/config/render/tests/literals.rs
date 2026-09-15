use super::*;

#[test]
fn a_hostile_literal_crosses_as_one_inert_string_and_never_as_structure() {
    // THE OTHER INJECTION CASE: a plain value, not a note. A quote could
    // close the string, a newline could open a heading or a key on the
    // next line, and a `#` could start a comment; escaped, all of it is
    // one line inside one basic string and parses back as itself.
    let hostile = "\"\n[evil]\nenabled = true\n# not a comment";
    let mut keys = toml::Table::new();
    keys.insert(
        "pns-events".to_string(),
        toml::Value::String(hostile.to_string()),
    );
    let mut hermes = toml::Table::new();
    hermes.insert("keys".to_string(), toml::Value::Table(keys));
    let mut plugins = toml::Table::new();
    plugins.insert("hermes".to_string(), toml::Value::Table(hermes));
    let mut values = toml::Table::new();
    values.insert("plugins".to_string(), toml::Value::Table(plugins));

    let text = render(&values).expect("a hostile literal renders");
    assert!(
        !text.lines().any(|line| line.starts_with("[evil]")),
        "the value opened a heading of its own: {text}"
    );
    let config = parse_config(&text).unwrap_or_else(|error| panic!("{error:?}\n{text}"));
    assert_eq!(
        config.plugins["hermes"].settings["keys"]["pns-events"].as_str(),
        Some(hostile)
    );
}

#[test]
fn a_literal_holding_a_chezmoi_action_opening_crosses_with_its_braces_broken_up() {
    // A LITERAL IS UNTRUSTED TEXT THIS RENDER MUST NEVER HAND CHEZMOI A LIVE
    // ACTION FROM: `quoted` is also what the closing prose relies on to keep
    // an eventual S2-generated `.tmpl` file inert wherever a value sits, so
    // `{{` and `}}` must never survive a quoted string as an adjacent pair.
    let hostile = "before{{ printf \"pwned\" }}after";
    let mut keys = toml::Table::new();
    keys.insert(
        "pns-events".to_string(),
        toml::Value::String(hostile.to_string()),
    );
    let mut hermes = toml::Table::new();
    hermes.insert("keys".to_string(), toml::Value::Table(keys));
    let mut plugins = toml::Table::new();
    plugins.insert("hermes".to_string(), toml::Value::Table(hermes));
    let mut values = toml::Table::new();
    values.insert("plugins".to_string(), toml::Value::Table(plugins));

    let text = render(&values).expect("a hostile literal renders");
    assert!(
        !text.contains("{{"),
        "a live action opening survived: {text}"
    );
    assert!(!text.contains("}}"), "a live action close survived: {text}");
    let config = parse_config(&text).unwrap_or_else(|error| panic!("{error:?}\n{text}"));
    assert_eq!(
        config.plugins["hermes"].settings["keys"]["pns-events"].as_str(),
        Some(hostile)
    );
}

/// A COLOUR IS A PAIR OF FLOATS, and nothing else in this schema is: the
/// renderer refused every float until `[lights.github]` existed, so this is
/// the arm that makes a stated colour reach the shipped file at all. The
/// refusal beside it is the other half: a type this schema has no spelling
/// for is still named rather than guessed at.
#[test]
fn a_colour_pair_renders_as_floats_and_a_type_with_no_spelling_is_still_refused() {
    let values = toml::toml! {
        [lights.github]
        pass = [0.2, 0.295]
        brightness = 60
    };
    let text = render(&values).expect("a colour pair renders");
    assert!(
        text.contains("pass = [0.2, 0.295]"),
        "the pair is written as the two numbers it is: {text}"
    );
    let config = parse_config(&text).unwrap_or_else(|error| panic!("{error:?}\n{text}"));
    assert_eq!(
        config.lights.expect("the render carries lights").github,
        pns_domain::lamps::config::Github {
            pulse: pns_domain::lamps::config::Pulse {
                duration_ms: 4000,
                brightness: 60,
            },
            pass: pns_domain::pulse::PulseColor { x: 0.2, y: 0.295 },
            fail: pns_domain::pulse::GITHUB_FAIL_COLOR,
        },
        "and it round trips through the parser that reads the shipped file"
    );
    // A WHOLE NUMBER STILL RENDERS AS A FLOAT, because TOML reads `1` as an
    // integer and the colour arm takes numbers only: dropping the fractional
    // part would write a file the parser then refuses.
    let ends = toml::toml! {
        [lights.github]
        fail = [1.0, 0.0]
    };
    assert!(
        render(&ends)
            .expect("the corners render")
            .contains("fail = [1.0, 0.0]"),
        "a trailing `.0` is what keeps the value a float"
    );
    // AND A TYPE THIS SCHEMA HAS NO SPELLING FOR IS REFUSED BY TYPE NAME.
    let dated = toml::toml! {
        [lights.github]
        pass = 1979-05-27T07:32:00Z
    };
    let refusal = render(&dated).expect_err("a datetime does not render");
    assert!(
        refusal.contains("datetime") && refusal.contains("does not render"),
        "{refusal}"
    );
    // A FLOAT THAT IS NOT A NUMBER IS REFUSED TOO. TOML spells `nan` and
    // `inf`, neither of which is a coordinate, and both would reach the file
    // as a literal the parser reads back as the same non-number.
    let infinite = toml::toml! {
        [lights.github]
        pass = [inf, 0.1]
    };
    let refusal = render(&infinite).expect_err("an infinity does not render");
    assert!(refusal.contains("finite"), "{refusal}");
}
