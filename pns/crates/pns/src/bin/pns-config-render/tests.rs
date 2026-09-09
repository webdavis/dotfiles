use super::{lookup, refuse_literal_secrets};

#[test]
fn a_literal_string_at_a_secret_bearing_path_is_refused_by_name() {
    let mut hue = toml::Table::new();
    hue.insert(
        "bridge".to_string(),
        toml::Value::String("192.168.1.9".to_string()),
    );
    let mut plugins = toml::Table::new();
    plugins.insert("hue".to_string(), toml::Value::Table(hue));
    let mut values = toml::Table::new();
    values.insert("plugins".to_string(), toml::Value::Table(plugins));

    let error = refuse_literal_secrets(&values)
        .expect_err("a literal bridge address is not a secret marker");
    assert!(error.contains("plugins.hue.bridge"), "{error}");
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
    plugins.insert("mobile".to_string(), toml::Value::Table(mobile));
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
    assert_eq!(lookup(&values, "plugins.hue.bridge"), None);
}
