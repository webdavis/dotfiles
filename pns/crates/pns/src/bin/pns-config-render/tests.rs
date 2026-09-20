use super::{lookup, refuse_literal_secrets};

#[test]
fn a_literal_string_at_a_secret_bearing_path_is_refused_by_name() {
    let mut hue = toml::Table::new();
    hue.insert(
        "bridge".to_string(),
        toml::Value::String("192.168.1.9".to_string()),
    );
    let mut plugins = toml::Table::new();
    plugins.insert("lights".to_string(), toml::Value::Table(hue));
    let mut values = toml::Table::new();
    values.insert("plugins".to_string(), toml::Value::Table(plugins));

    let error = refuse_literal_secrets(&values)
        .expect_err("a literal bridge address is not a secret marker");
    assert!(error.contains("plugins.lights.bridge"), "{error}");
}

#[test]
fn a_proper_secret_marker_table_is_accepted() {
    let mut marker = toml::Table::new();
    marker.insert(
        "keepassxc".to_string(),
        toml::Value::String("Some Entry".to_string()),
    );
    marker.insert(
        "field".to_string(),
        toml::Value::String("Password".to_string()),
    );
    let mut mobile = toml::Table::new();
    mobile.insert("token".to_string(), toml::Value::Table(marker));
    let mut plugins = toml::Table::new();
    plugins.insert("phone".to_string(), toml::Value::Table(mobile));
    let mut values = toml::Table::new();
    values.insert("plugins".to_string(), toml::Value::Table(plugins));

    refuse_literal_secrets(&values).expect("a well-shaped secret marker is accepted");
}

#[test]
fn an_absent_secret_bearing_key_is_accepted() {
    refuse_literal_secrets(&toml::Table::new()).expect("nothing present, nothing to refuse");
}

#[test]
fn lookup_stops_at_a_non_table_segment_rather_than_panicking() {
    let mut values = toml::Table::new();
    values.insert(
        "plugins".to_string(),
        toml::Value::String("not a table".to_string()),
    );
    assert_eq!(lookup(&values, "plugins.lights.bridge"), None);
}

/// EVERY CHANNEL ID IS A SECRET, so every key of the open
/// `[plugins.log.channels]` table is secret-bearing and not only the
/// fixed `default` one. A pasted id under a project's key is the exact
/// mistake the values file exists to make impossible.
///
/// The id below is an obvious fake; a real one lives in KeePassXC and reaches
/// no committed file.
#[test]
fn a_literal_channel_id_under_any_project_key_is_refused_by_name() {
    let mut channels = toml::Table::new();
    channels.insert(
        "dotfiles".to_string(),
        toml::Value::String("000000000000000000".to_string()),
    );
    let mut log = toml::Table::new();
    log.insert("channels".to_string(), toml::Value::Table(channels));
    let mut plugins = toml::Table::new();
    plugins.insert("log".to_string(), toml::Value::Table(log));
    let mut values = toml::Table::new();
    values.insert("plugins".to_string(), toml::Value::Table(plugins));

    let error =
        refuse_literal_secrets(&values).expect_err("a literal channel id is not a secret marker");
    assert!(error.contains("plugins.log.channels.dotfiles"), "{error}");
}
