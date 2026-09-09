use super::*;

#[test]
fn a_hostile_literal_crosses_as_one_inert_string_and_never_as_structure() {
    // THE OTHER INJECTION CASE: a plain value, not a note. A quote could
    // close the string, a newline could open a heading or a key on the
    // next line, and a `#` could start a comment; escaped, all of it is
    // one line inside one basic string and parses back as itself.
    let hostile = "\"\n[evil]\nenabled = true\n# not a comment";
    let mut hermes = toml::Table::new();
    hermes.insert("key".to_string(), toml::Value::String(hostile.to_string()));
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
        config.plugins["hermes"].settings["key"].as_str(),
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
    let mut hermes = toml::Table::new();
    hermes.insert("key".to_string(), toml::Value::String(hostile.to_string()));
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
        config.plugins["hermes"].settings["key"].as_str(),
        Some(hostile)
    );
}
