use super::*;

#[test]
fn a_mistyped_key_inside_a_plugin_table_is_refused_naming_the_table_and_the_key() {
    // A plugin's settings used to reach the plugin free-form, so a near
    // miss was a destination that quietly never worked: `room` for `rooms`
    // is a pulse into a room the bridge does not have, and `tokens` for
    // `token` is a phone card that silently never leaves the machine.
    for (table, mistyped, near) in [
        ("plugins.hermes", "keys", "key"),
        ("plugins.hue", "room", "rooms"),
        ("plugins.macos-banner", "sound", "enabled"),
        ("plugins.mobile", "tokens", "token"),
        ("plugins.router", "phone", "device_hostname"),
    ] {
        let said = refusal(&format!("[{table}]\nenabled = true\n{mistyped} = \"x\"\n"));
        assert!(
            said.contains(&format!("`{table}`")),
            "the TABLE is named: {said}"
        );
        assert!(
            said.contains(&format!("`{mistyped}`")),
            "and so is the key: {said}"
        );
        assert!(
            said.contains(near),
            "and the keys it does serve are listed: {said}"
        );
    }
}

#[test]
fn every_key_a_shipped_plugin_table_serves_is_still_admitted() {
    // The positive control under the refusal above: a sweep that refused
    // the whole vocabulary would pass every assertion up there.
    let shipped = "[plugins.hermes]\nenabled = true\nkey = \"k\"\n             [plugins.hue]\nenabled = true\nbridge = \"b\"\nkey = \"k\"\n             rooms = [\"3F - Studio\"]\nquiet_hours = \"22:00-07:00\"\n             [plugins.macos-banner]\nenabled = true\n             [plugins.mobile]\nenabled = true\ntype = \"moshi\"\ntoken = \"t\"\n             mobile_watch_card = false\nsubmit_deadline_secs = 5\n             [plugins.router]\nenabled = true\ntype = \"unifi\"\n             router_url = \"https://192.168.1.1\"\ndevice_hostname = \"mister\"\n             device_mac = \"2e:11:ab:6d:b0:4f\"\ndevice_ipv4 = \"192.168.1.9\"\n             api_key = \"k\"\nstale_alert_channel = \"priority\"\n";
    let config = parse_config(shipped).expect("every shipped key parses");
    assert_eq!(config.plugins.len(), 5);
}

#[test]
fn an_unregistered_plugin_tables_settings_stay_free_form_because_selection_is_by_name() {
    // TODAY'S BEHAVIOUR, pinned rather than changed. This layer knows the
    // vocabulary of the plugins that ship and has none for a name nothing
    // registered, so judging its keys would mean inventing a schema for a
    // plugin that does not exist. The NAME is the defect and the registry
    // is where it is refused, which is one layer later and still loud.
    let config = parse_config("[plugins.nosuch]\nenabled = true\nwhatever = 1\n")
        .expect("the settings of an unknown plugin are not this layer's to judge");
    assert!(config.plugins["nosuch"].enabled);
    assert!(config.plugins["nosuch"].settings.contains_key("whatever"));
    assert!(
        pns_domain::registry::roster()
            .enabled(&config.plugin_switches())
            .is_err(),
        "and the name itself is still refused, one layer on"
    );
}

#[test]
fn a_table_the_file_does_not_serve_is_refused_listing_the_tables_it_does() {
    // THE MOST OPERATOR-VISIBLE TYPO CLASS: a whole table misspelled, or a
    // table that moved. `[home]` is the real one; the router probe's
    // settings moved under `[plugins.router]`, and a config written before
    // that move is refused WHOLE, which takes every plugin's secret with
    // it. Told only that `home` is unknown, an operator has nowhere to go.
    let said = refusal("[home]\nrouter_url = \"https://192.168.1.1\"\n");
    assert!(said.contains("`home`"), "the table is named: {said}");
    for serves in ["daemon", "focus", "lights", "nag", "plugins", "recap"] {
        assert!(
            said.contains(serves),
            "and `{serves}` is among the tables it says the file serves: {said}"
        );
    }
}

#[test]
fn type_is_the_word_that_selects_a_backend_and_the_old_brand_is_refused() {
    // ONE WORD FOR ONE QUESTION, under every table that has a backend to
    // pick. `brand` was the router's alone, so an operator who had learnt
    // it on one table had to learn a second word on the next; there is now
    // one, and the retired spelling is refused by name with the vocabulary
    // spelled out rather than reaching the probe as a setting it ignores.
    let said = refusal("[plugins.router]\nenabled = true\nbrand = \"unifi\"\n");
    assert!(said.contains("`brand`"), "the retired key is named: {said}");
    assert!(
        said.contains("type"),
        "and `type` is listed instead: {said}"
    );
    assert!(
        parse_config("[plugins.router]\nenabled = true\ntype = \"unifi\"\n").is_ok(),
        "the router table serves `type`"
    );
    assert!(
        parse_config("[plugins.mobile]\nenabled = true\ntype = \"moshi\"\n").is_ok(),
        "and so does the mobile table"
    );
}

#[test]
fn every_table_refuses_an_unknown_key_by_name_and_lists_what_it_serves() {
    // ONE TEST PER TABLE, driven by the roster rather than written out, so
    // a table added to the schema without this treatment is a red test.
    // THE TOP LEVEL IS ONE OF THE ROWS, so the outermost refusal is held to
    // the same standard as the innermost.
    for (table, serves) in super::super::TABLE_KEYS.iter().copied() {
        let said = refusal(&config_writing(table, "zzz_not_a_key", "\"x\""));
        assert!(
            said.contains(&refusal_names(table)) && said.contains("`zzz_not_a_key`"),
            "`{table}` names the table and the key: {said}"
        );
        for key in serves {
            assert!(
                said.contains(key),
                "`{table}` lists `{key}` among what it serves: {said}"
            );
        }
    }
}
