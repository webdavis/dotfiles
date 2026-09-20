use super::*;

#[test]
fn an_unknown_key_is_refused_by_name_wherever_it_appears() {
    // A TOP-LEVEL KEY, A PLUGIN NAME, A KEY INSIDE A TABLE, AND A KEY
    // INSIDE A TARGET DECLARATION: the same leftover check runs after
    // every table this walk writes, so a values file cannot smuggle any
    // of the four past it.
    let mut top_level = toml::Table::new();
    top_level.insert("zzz_not_a_key".to_string(), toml::Value::Boolean(true));
    let error = render(&top_level).expect_err("an unknown top-level key must be refused");
    assert!(error.contains("zzz_not_a_key"), "{error}");

    let mut plugins = toml::Table::new();
    plugins.insert(
        "zzz_not_a_plugin".to_string(),
        toml::Value::Table(toml::Table::new()),
    );
    let mut values = toml::Table::new();
    values.insert("plugins".to_string(), toml::Value::Table(plugins));
    let error = render(&values).expect_err("an unknown plugin name must be refused");
    assert!(error.contains("zzz_not_a_plugin"), "{error}");

    let mut daemon = toml::Table::new();
    daemon.insert("zzz_not_a_key".to_string(), toml::Value::Boolean(true));
    let mut values = toml::Table::new();
    values.insert("daemon".to_string(), toml::Value::Table(daemon));
    let error = render(&values).expect_err("an unknown key inside a table must be refused");
    assert!(error.contains("zzz_not_a_key"), "{error}");

    let mut target = toml::Table::new();
    target.insert("zzz_not_a_key".to_string(), toml::Value::Boolean(true));
    let mut rooms = toml::Table::new();
    rooms.insert("Studio".to_string(), toml::Value::Table(target));
    let mut lights = toml::Table::new();
    lights.insert("room".to_string(), toml::Value::Table(rooms));
    let mut values = toml::Table::new();
    values.insert("lights".to_string(), toml::Value::Table(lights));
    let error = render(&values).expect_err("an unknown key inside a target must be refused");
    assert!(error.contains("zzz_not_a_key"), "{error}");
}

#[test]
fn an_unknown_table_is_refused_by_name() {
    let mut lights = toml::Table::new();
    lights.insert(
        "zzz_not_a_table".to_string(),
        toml::Value::Table(toml::Table::new()),
    );
    let mut values = toml::Table::new();
    values.insert("lights".to_string(), toml::Value::Table(lights));
    let error = render(&values).expect_err("an unknown lights sub-table must be refused");
    assert!(error.contains("zzz_not_a_table"), "{error}");
}

#[test]
fn an_opt_in_table_absent_renders_commented_and_present_renders_live() {
    // ABSENT: the heading, `enabled` and every key are commented, and the
    // table never reaches the parsed config at all. `enabled` is commented
    // AT ITS OWN DEFAULT, which for a plugin is off.
    let text = render(&toml::Table::new()).expect("an empty walk still renders");
    assert!(
        text.contains("# [plugins.log]\n# enabled = false\n"),
        "{text}"
    );
    let config = parse_config(&text).unwrap_or_else(|error| panic!("{error:?}\n{text}"));
    assert!(!config.plugins.contains_key("hermes"));

    // PRESENT AND ARMED: the switch is the caller's own statement rather
    // than the render's reading of a table having shown up, so a plugin is
    // on in the file because a line says so.
    let mut log = toml::Table::new();
    log.insert("enabled".to_string(), toml::Value::Boolean(true));
    let mut keys = toml::Table::new();
    keys.insert(
        "pns-events".to_string(),
        toml::Value::String("secret".to_string()),
    );
    log.insert("keys".to_string(), toml::Value::Table(keys));
    let mut plugins = toml::Table::new();
    plugins.insert("log".to_string(), toml::Value::Table(log));
    let mut values = toml::Table::new();
    values.insert("plugins".to_string(), toml::Value::Table(plugins));
    let text = render(&values).expect("an armed table renders");
    assert!(text.contains("[plugins.log]\nenabled = true\n"), "{text}");
    let config = parse_config(&text).unwrap_or_else(|error| panic!("{error:?}\n{text}"));
    assert!(config.plugins["hermes"].enabled);
}

#[test]
fn a_rendered_presence_block_parses_back_and_the_registry_selects_the_sensor() {
    // THE WHOLE ROUND TRIP, because the three statements of this table (the
    // layout that writes it, the roster that admits its keys, and the
    // registry that selects its name) are three edits and nothing else
    // holds them together.
    let mut presence = toml::Table::new();
    presence.insert(
        "rooms".to_string(),
        toml::Value::Array(vec![toml::Value::String("3F - Studio".to_string())]),
    );
    presence.insert("enabled".to_string(), toml::Value::Boolean(true));
    let mut hue = toml::Table::new();
    hue.insert("enabled".to_string(), toml::Value::Boolean(true));
    let mut plugins = toml::Table::new();
    plugins.insert("presence".to_string(), toml::Value::Table(presence));
    plugins.insert("lights".to_string(), toml::Value::Table(hue));
    let mut values = toml::Table::new();
    values.insert("plugins".to_string(), toml::Value::Table(plugins));

    let text = render(&values).expect("an armed presence table renders");
    let config = parse_config(&text).unwrap_or_else(|error| panic!("{error:?}\n{text}"));
    let settings = crate::config::parse_presence(&config)
        .unwrap_or_else(|error| panic!("{error:?}\n{text}"))
        .expect("the table is on");
    assert_eq!(settings.rooms, vec!["3F - Studio".to_string()]);
    assert_eq!(
        (settings.poll_interval_secs, settings.reading_max_age_secs),
        (5, 15),
        "the written defaults are the ones the reader takes"
    );
    let selected: Vec<&str> = pns_domain::registry::roster()
        .enabled(&config.plugin_switches())
        .expect("hue is written beside it")
        .iter()
        .map(|entry| entry.name)
        .collect();
    assert!(selected.contains(&"presence"), "{selected:?}");
}
