use super::*;

fn parsed(text: &str) -> Result<pns_domain::routes::Routes, ConfigError> {
    parse_routes(text.parse::<toml::Table>().expect("valid TOML").into())
}

#[test]
fn both_names_are_read_from_the_table() {
    let routes = parsed("default = \"logbook\"\nurgent = \"sirens\"\n").expect("a routes table");
    assert_eq!(routes.default_route(), "logbook");
    assert_eq!(routes.urgent_route(), "sirens");
}

#[test]
fn a_key_the_table_leaves_out_keeps_its_shipped_default() {
    let routes = parsed("urgent = \"sirens\"\n").expect("a routes table");
    assert_eq!(routes.default_route(), "pns-events");
    assert_eq!(routes.urgent_route(), "sirens");
    assert_eq!(
        parsed("").expect("an empty table"),
        pns_domain::routes::Routes::default(),
        "an empty table is the shipped pair"
    );
}

#[test]
fn a_name_no_url_could_carry_is_refused_by_key_rather_than_defaulted() {
    // THE MUTANT THIS PINS: the usable-name check removed. A route with a
    // slash in it would build a URL pointing at a path nobody prepared, and
    // the page would be gone rather than refused.
    for text in [
        "default = \"webhooks/pns\"\n",
        "urgent = \"\"\n",
        "urgent = \"sirens!\"\n",
    ] {
        let refusal = parsed(text).expect_err(text);
        assert!(
            refusal.detail().contains("usable route name"),
            "{text}: {refusal:?}"
        );
    }
}

#[test]
fn a_non_string_name_and_an_unknown_key_are_each_refused_by_name() {
    let refusal = parsed("default = 42\n").expect_err("a number is no route");
    assert!(refusal.detail().contains("not a string"), "{refusal:?}");
    let refusal = parsed("priority = \"sirens\"\n").expect_err("no such key");
    assert!(refusal.detail().contains("`priority`"), "{refusal:?}");
}
