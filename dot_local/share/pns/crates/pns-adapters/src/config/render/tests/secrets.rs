use super::*;

#[test]
fn a_secret_marker_renders_as_the_chezmoi_action_and_a_literal_renders_quoted() {
    let mut mobile = toml::Table::new();
    mobile.insert(
        "token".to_string(),
        secret("Moshi :: Webhook Secret", "Password"),
    );
    let mut plugins = toml::Table::new();
    plugins.insert("mobile".to_string(), toml::Value::Table(mobile));
    let mut values = toml::Table::new();
    values.insert("plugins".to_string(), toml::Value::Table(plugins));

    let text = render(&values).expect("a secret marker renders");
    assert!(
        text.contains("token = {{ (keepassxc \"Moshi :: Webhook Secret\").Password | toToml }}"),
        "{text}"
    );
    // AND A LITERAL RENDERS QUOTED, right beside it: `type` still comes
    // off the layout's own `Default`, escaped the ordinary way.
    assert!(text.contains("type = \"moshi\""), "{text}");

    // A RENDERED SECRET IS NOT TOML: the action carries no author quotes
    // of its own, since `toToml` is what supplies them once chezmoi
    // substitutes the vault value, so the stub's placeholder has to
    // supply a quoted string in its place before the whole file can
    // parse.
    let rendered =
        crate::config::strip_chezmoi_actions(&text, |_, _| "\"from-the-vault\"".to_string())
            .expect("a chezmoi-stub round trip stands in for a well-formed secret action");
    let config = parse_config(&rendered).unwrap_or_else(|error| panic!("{error:?}\n{rendered}"));
    assert_eq!(
        config.plugins["mobile"].settings["token"].as_str(),
        Some("from-the-vault")
    );
}

#[test]
fn a_username_secret_marker_renders_the_exact_action_and_round_trips_through_the_stub() {
    // SECRET_FIELDS HOLDS TWO NAMES, and hue's key is the shipped table
    // that actually needs the second one: a `SECRET_FIELDS` narrowed to
    // `Password` alone would pass every other test in this module, since
    // none of them exercises `UserName`.
    let mut hue = toml::Table::new();
    hue.insert(
        "bridge".to_string(),
        toml::Value::String("192.168.1.9".to_string()),
    );
    hue.insert("key".to_string(), secret("Hue Bridge", "UserName"));
    hue.insert(
        "rooms".to_string(),
        toml::Value::Array(vec![toml::Value::String("Studio".to_string())]),
    );
    let mut plugins = toml::Table::new();
    plugins.insert("hue".to_string(), toml::Value::Table(hue));
    let mut values = toml::Table::new();
    values.insert("plugins".to_string(), toml::Value::Table(plugins));

    let text = render(&values).expect("a UserName secret marker renders");
    assert!(
        text.contains("key = {{ (keepassxc \"Hue Bridge\").UserName | toToml }}"),
        "{text}"
    );
    let rendered =
        crate::config::strip_chezmoi_actions(&text, |_, _| "\"from-the-vault\"".to_string())
            .expect("a chezmoi-stub round trip stands in for a well-formed secret action");
    let config = parse_config(&rendered).unwrap_or_else(|error| panic!("{error:?}\n{rendered}"));
    assert_eq!(
        config.plugins["hue"].settings["key"].as_str(),
        Some("from-the-vault")
    );
}

#[test]
fn a_secret_holding_a_quote_and_a_backslash_round_trips_through_the_totoml_stub() {
    // WHAT `toToml` ACTUALLY EMITS for the byte sequence `a"b\c`, per the
    // sol-1 probe table: `"a\"b\\c"`. The stub stands in for chezmoi
    // having already run `| toToml` on the vault value, so it is handed
    // that exact TOML text rather than the raw secret.
    let mut mobile = toml::Table::new();
    mobile.insert(
        "token".to_string(),
        secret("Quote Backslash Secret", "Password"),
    );
    let mut plugins = toml::Table::new();
    plugins.insert("mobile".to_string(), toml::Value::Table(mobile));
    let mut values = toml::Table::new();
    values.insert("plugins".to_string(), toml::Value::Table(plugins));

    let text = render(&values).expect("a secret marker renders");
    let rendered =
        crate::config::strip_chezmoi_actions(&text, |_, _| "\"a\\\"b\\\\c\"".to_string())
            .expect("a chezmoi-stub round trip stands in for a well-formed secret action");
    let config = parse_config(&rendered).unwrap_or_else(|error| panic!("{error:?}\n{rendered}"));
    assert_eq!(
        config.plugins["mobile"].settings["token"].as_str(),
        Some("a\"b\\c")
    );
}

#[test]
fn a_plain_secret_round_trips_through_the_totoml_stub_too() {
    let mut mobile = toml::Table::new();
    mobile.insert("token".to_string(), secret("Plain Secret", "Password"));
    let mut plugins = toml::Table::new();
    plugins.insert("mobile".to_string(), toml::Value::Table(mobile));
    let mut values = toml::Table::new();
    values.insert("plugins".to_string(), toml::Value::Table(plugins));

    let text = render(&values).expect("a secret marker renders");
    let rendered = crate::config::strip_chezmoi_actions(&text, |_, _| "\"plain\"".to_string())
        .expect("a chezmoi-stub round trip stands in for a well-formed secret action");
    let config = parse_config(&rendered).unwrap_or_else(|error| panic!("{error:?}\n{rendered}"));
    assert_eq!(
        config.plugins["mobile"].settings["token"].as_str(),
        Some("plain")
    );
}

#[test]
fn a_secret_tables_unknown_member_is_named_rather_than_only_counted() {
    // `table.len() != 2` ALONE only counts members, so `{ keepassxc,
    // field, typo }` reports the pair-count rule and never says which
    // key does not belong: naming the offender needs its own check.
    let mut table = toml::Table::new();
    table.insert(
        "keepassxc".to_string(),
        toml::Value::String("entry".to_string()),
    );
    table.insert(
        "field".to_string(),
        toml::Value::String("Password".to_string()),
    );
    table.insert("typo".to_string(), toml::Value::String("oops".to_string()));
    let error = super::secret_action(&table).expect_err("an unknown secret member must be refused");
    assert!(error.contains("typo"), "{error}");
}

#[test]
fn a_secrets_field_is_whitelisted_to_the_two_chezmoi_methods() {
    let error = super::secret_action(
        secret("Moshi :: Webhook Secret", "Notes")
            .as_table()
            .unwrap(),
    )
    .expect_err("Notes is not a field keepassxc exposes to chezmoi");
    assert!(error.contains("Notes"), "{error}");
}
