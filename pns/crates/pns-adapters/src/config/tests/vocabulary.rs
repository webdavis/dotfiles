use super::*;

#[test]
fn a_mistyped_key_inside_a_plugin_table_is_refused_naming_the_table_and_the_key() {
    // A plugin's settings used to reach the plugin free-form, so a near
    // miss was a destination that quietly never worked: `room` for `rooms`
    // is a pulse into a room the bridge does not have, and `tokens` for
    // `token` is a phone card that silently never leaves the machine.
    for (table, mistyped, near) in [
        ("plugins.hermes", "key", "keys"),
        ("plugins.lights", "room", "rooms"),
        ("plugins.banner", "sound", "enabled"),
        ("plugins.mobile", "tokens", "token"),
        ("plugins.home_presence", "phone", "device_hostname"),
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
    let shipped = "[plugins.hermes]\nenabled = true\n[plugins.hermes.keys]\npns-events = \"k\"\n             posture-pages = \"k\"\npriority = \"k\"\n             [plugins.lights]\nenabled = true\nbridge = \"b\"\nkey = \"k\"\n             rooms = [\"3F - Studio\"]\nquiet_hours = \"22:00-07:00\"\n             [plugins.banner]\nenabled = true\n             [plugins.mobile]\nenabled = true\ntype = \"moshi\"\ntoken = \"t\"\n             mobile_watch_card = false\nsubmit_deadline_secs = 5\n             [plugins.home_presence]\nenabled = true\ntype = \"unifi\"\n             router_url = \"https://192.168.1.1\"\ndevice_hostname = \"mister\"\n             device_mac = \"2e:11:ab:6d:b0:4f\"\ndevice_ipv4 = \"192.168.1.9\"\n             api_key = \"k\"\nstale_alert_channel = \"priority\"\n";
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
    // settings moved under `[plugins.home_presence]`, and a config written before
    // that move is refused WHOLE, which takes every plugin's secret with
    // it. Told only that `home` is unknown, an operator has nowhere to go.
    let said = refusal("[home]\nrouter_url = \"https://192.168.1.1\"\n");
    assert!(said.contains("`home`"), "the table is named: {said}");
    for serves in [
        "daemon", "delivery", "focus", "lights", "plugins", "recap", "remind", "stale",
    ] {
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
    let said = refusal("[plugins.home_presence]\nenabled = true\nbrand = \"unifi\"\n");
    assert!(said.contains("`brand`"), "the retired key is named: {said}");
    assert!(
        said.contains("type"),
        "and `type` is listed instead: {said}"
    );
    assert!(
        parse_config("[plugins.home_presence]\nenabled = true\ntype = \"unifi\"\n").is_ok(),
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
        if crate::config::schema::is_open(table) {
            // AN OPEN TABLE CANNOT REFUSE A KEY, and that is the trade the
            // channel map makes: its vocabulary is the operator's project
            // names, so there is no roster to check one against. The key it
            // cannot do without, `default`, is required by
            // `refusals::refuse_a_map_without_a_catch_all` instead.
            assert!(
                parse_config(&config_writing(table, "zzz_not_a_key", "\"x\"")).is_ok(),
                "`{table}` is open and takes any key"
            );
            continue;
        }
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

#[test]
fn a_key_under_any_route_name_the_gateway_serves_is_accepted() {
    // THE MUTANT THIS PINS: the compiled route roster restored. Until
    // 2026-09-15 this table was checked against a list of three names in
    // pns's own source, which made one deployment's gateway a condition of
    // the product loading: every name below was refused at load. The keys ARE
    // the roster now, so a route this repository has never heard of works.
    //
    // `general` is the real case: hermes serves that route, and a machine
    // that wants its own events there says so here. `weather-balloons` is a
    // route nothing in this repository will ever mention.
    for route in ["general", "weather-balloons", "pns_events_2"] {
        let text = format!(
            "[plugins.hermes]\nenabled = true\n[plugins.hermes.keys]\n{route} = \"secret\"\n"
        );
        let config = crate::config::parse_config(&text)
            .unwrap_or_else(|error| panic!("`{route}` was refused: {error:?}"));
        let settings = &config.plugins["hermes"].settings;
        assert_eq!(
            crate::config::hermes_keys(settings).key_for(route),
            Some("secret"),
            "`{route}` parsed and then granted no key"
        );
    }
}

#[test]
fn a_route_name_no_url_could_carry_still_signs_nothing() {
    // AND THE SAFETY PROPERTY THE OLD ROSTER WAS CHECKED FOR SURVIVES: a name
    // pns cannot build a URL out of is one nothing posts to, so the key it
    // holds signs nothing rather than signing for a route nobody granted.
    let config = crate::config::parse_config(
        "[plugins.hermes]\nenabled = true\n[plugins.hermes.keys]\n\"a/b\" = \"secret\"\n",
    )
    .expect("an unusable name is the operator's to write");
    assert!(
        !pns_domain::safety::route_name_is_usable("a/b"),
        "the name this case is about became usable"
    );
    assert_eq!(
        crate::config::hermes_keys(&config.plugins["hermes"].settings).key_for("pns-events"),
        None,
        "the unusable name granted the default route a key"
    );
}
