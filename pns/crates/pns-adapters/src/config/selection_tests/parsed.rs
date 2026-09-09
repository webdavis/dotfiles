use super::*;

// --- plugin kinds -------------------------------------------------------

#[test]
fn a_sensor_registers_by_name_so_a_typo_near_it_is_still_refused() {
    // A sensor carries no routing, but it occupies a config table like any
    // channel, so the registry has to know its name or the unknown-name
    // refusal would call the operator's correct spelling a typo.
    let mut registry = Registry::new();
    registry
        .register_channel("hermes", hermes_routing())
        .unwrap();
    registry.register_sensor("router").unwrap();
    assert_eq!(registry.names(), vec!["hermes", "router"]);

    let typo = parse_config("[plugins.rotuer]\nenabled = true\n").unwrap();
    assert_eq!(
        registry.enabled(&typo.plugin_switches()),
        Err(RegistryError::UnknownPlugin("rotuer".to_string()))
    );
}

// --- selection by config ------------------------------------------------

#[test]
fn a_presence_table_switched_on_without_hue_is_refused_naming_both() {
    // Presence reads the bridge through `[plugins.hue]`'s own address and
    // key. Selected without it, the sensor is a table the operator turned
    // on that could never take a reading, and a silent one is worse than
    // the refusal: the fix is in the OTHER table, so both are named.
    let config = parse_config("[plugins.presence]\nenabled = true\ntype = \"hue\"\n").unwrap();
    assert_eq!(
        roster().enabled(&config.plugin_switches()),
        Err(RegistryError::Unsatisfied {
            plugin: "presence".to_string(),
            needs: "hue".to_string(),
        })
    );
}

#[test]
fn a_hue_table_switched_off_refuses_presence_just_as_an_absent_one_does() {
    let config = parse_config(
        "[plugins.presence]\nenabled = true\ntype = \"hue\"\n\
         [plugins.hue]\nenabled = false\n",
    )
    .unwrap();
    assert!(matches!(
        roster().enabled(&config.plugin_switches()),
        Err(RegistryError::Unsatisfied { .. })
    ));
}

#[test]
fn presence_is_selected_once_hue_carries_the_bridge_it_reads() {
    let config = parse_config(
        "[plugins.presence]\nenabled = true\ntype = \"hue\"\n\
         [plugins.hue]\nenabled = true\n",
    )
    .unwrap();
    let names: Vec<&str> = roster()
        .enabled(&config.plugin_switches())
        .expect("hue carries it")
        .iter()
        .map(|entry| entry.name)
        .collect();
    assert_eq!(names, vec!["presence", "hue"]);
}

#[test]
fn the_config_selects_and_registration_order_beats_config_order() {
    // The config lists banner before mobile; the plan order is still the
    // registered one, because delivery order is policy, not preference.
    let config =
        parse_config("[plugins.macos-banner]\nenabled = true\n[plugins.mobile]\nenabled = true\n")
            .unwrap();
    let enabled = roster().enabled(&config.plugin_switches()).unwrap();
    let names: Vec<&str> = enabled.iter().map(|r| r.name).collect();
    assert_eq!(names, vec!["mobile", "macos-banner"]);
}

#[test]
fn a_disabled_or_omitted_plugin_is_simply_not_selected() {
    let config =
        parse_config("[plugins.mobile]\nenabled = true\n[plugins.hermes]\nenabled = false\n")
            .unwrap();
    let enabled = roster().enabled(&config.plugin_switches()).unwrap();
    let names: Vec<&str> = enabled.iter().map(|r| r.name).collect();
    assert_eq!(names, vec!["mobile"]);
}

#[test]
fn an_enabled_name_nothing_registered_is_refused_naming_it() {
    // A typo'd plugin name that silently no-ops is a notification quietly
    // turned off; the registry refuses it the way the config layer
    // refuses unknown keys.
    let config = parse_config("[plugins.mosih]\nenabled = true\n").unwrap();
    assert_eq!(
        roster().enabled(&config.plugin_switches()),
        Err(RegistryError::UnknownPlugin("mosih".to_string()))
    );
}

#[test]
fn a_disabled_unknown_name_is_still_refused_because_the_typo_is_the_defect() {
    // `enabled = false` on an unknown table is the same typo one edit
    // away from silently disabling a real plugin; refuse it now, while
    // the operator is looking at the file they just edited.
    let config = parse_config("[plugins.mosih]\nenabled = false\n").unwrap();
    assert_eq!(
        roster().enabled(&config.plugin_switches()),
        Err(RegistryError::UnknownPlugin("mosih".to_string()))
    );
}

#[test]
fn a_config_that_enables_a_sensor_selects_it_alongside_the_channels() {
    // A sensor is selected by an ordinary `[plugins.<name>]` table, so the
    // config layer needs no idea that kinds exist. hermes rides along as
    // the positive control: a selection that dropped everything would
    // fail this too.
    let mut registry = Registry::new();
    registry
        .register_channel("hermes", hermes_routing())
        .unwrap();
    registry.register_sensor("router").unwrap();

    let both = parse_config("[plugins.router]\nenabled = true\n[plugins.hermes]\nenabled = true\n")
        .unwrap();
    let selection = registry.enabled(&both.plugin_switches()).unwrap();
    let names: Vec<&str> = selection.iter().map(|r| r.name).collect();
    assert_eq!(names, vec!["hermes", "router"]);

    // And `enabled = false` turns a sensor off like anything else: no kind
    // is quietly always-on.
    let off = parse_config("[plugins.router]\nenabled = false\n[plugins.hermes]\nenabled = true\n")
        .unwrap();
    let selection = registry.enabled(&off.plugin_switches()).unwrap();
    let names: Vec<&str> = selection.iter().map(|r| r.name).collect();
    assert_eq!(names, vec!["hermes"]);
}

#[test]
fn an_empty_config_selects_nothing_which_is_a_verdict_not_an_error() {
    let config = parse_config("").unwrap();
    let enabled = roster().enabled(&config.plugin_switches()).unwrap();
    assert!(enabled.is_empty());
}
