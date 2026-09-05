//! The registry's own tests: what registration refuses, what the one
//! constructor builds, what the compiled-in roster declares, and what the
//! census reports.
//!
//! The tests that drive selection from a PARSED CONFIG stay in the legacy
//! package beside `select_plugins`: this crate takes no TOML, and those tests
//! build a real config on purpose. The three routing fixtures and the
//! three-plugin registry below are spelled there too for that reason, until
//! the config edge moves and the two halves rejoin.

use super::{PluginKind, Registration, Registry, RegistryError, Routing, build_registry};

const REMOTE_GATED: Routing = Routing {
    local: false,
    presence_gated: true,
    durable: false,
    event_dispatched: true,
};
const DURABLE: Routing = Routing {
    local: false,
    presence_gated: false,
    durable: true,
    event_dispatched: true,
};
const LOCAL: Routing = Routing {
    local: true,
    presence_gated: false,
    durable: false,
    event_dispatched: true,
};

fn three_plugin_registry() -> Registry {
    let mut registry = Registry::new();
    registry.register_channel("mobile", REMOTE_GATED).unwrap();
    registry.register_channel("hermes", DURABLE).unwrap();
    registry.register_channel("macos-banner", LOCAL).unwrap();
    registry
}

// --- registration -------------------------------------------------------

#[test]
fn registration_order_is_kept_because_it_is_delivery_order() {
    let registry = three_plugin_registry();
    assert_eq!(registry.names(), vec!["mobile", "hermes", "macos-banner"]);
}

#[test]
fn a_name_already_taken_is_refused_naming_it() {
    let mut registry = Registry::new();
    registry.register_channel("mobile", REMOTE_GATED).unwrap();
    assert_eq!(
        registry.register_channel("mobile", LOCAL),
        Err(RegistryError::Duplicate("mobile".to_string()))
    );
}

#[test]
fn one_name_cannot_be_two_kinds_because_they_share_one_config_table_space() {
    // `[plugins.router]` names exactly one plugin. If a sensor could take
    // a channel's name the config would select both and the operator
    // would have no way to say which they meant, so the second claim is
    // refused in either order, naming the offender.
    let mut sensor_first = Registry::new();
    sensor_first.register_sensor("router").unwrap();
    assert_eq!(
        sensor_first.register_channel("router", LOCAL),
        Err(RegistryError::Duplicate("router".to_string()))
    );

    let mut channel_first = Registry::new();
    channel_first.register_channel("router", LOCAL).unwrap();
    assert_eq!(
        channel_first.register_sensor("router"),
        Err(RegistryError::Duplicate("router".to_string()))
    );

    let mut twice = Registry::new();
    twice.register_sensor("router").unwrap();
    assert_eq!(
        twice.register_sensor("router"),
        Err(RegistryError::Duplicate("router".to_string()))
    );

    // The refusal is what keeps the roster from growing a second entry,
    // not a silent overwrite of the first.
    assert_eq!(sensor_first.names(), vec!["router"]);
    assert_eq!(channel_first.names(), vec!["router"]);
    assert_eq!(twice.names(), vec!["router"]);
}

// --- the one roster constructor -----------------------------------------

#[test]
fn a_registry_is_built_from_a_slice_of_declarations_in_the_order_given() {
    // ONE constructor, taking the declarations as data. Two hand-written
    // loops over the same const diverge the moment one of them grows a
    // kind the other does not know about; a slice in and a registry out
    // makes that unrepresentable, and lets a test hand it a roster of its
    // own without the composition root's four entries in the way.
    let entries = [
        Registration {
            name: "hermes",
            kind: PluginKind::Channel(DURABLE),
        },
        Registration {
            name: "router",
            kind: PluginKind::Sensor,
        },
    ];
    assert_eq!(build_registry(&entries).names(), vec!["hermes", "router"]);
}

#[test]
#[should_panic(expected = "router")]
fn a_roster_that_claims_one_name_twice_panics_naming_it() {
    // The only reachable duplicate is in a compiled-in const, so it is
    // deterministic: it fires on the first call in every mode and every
    // test run and cannot reach an operator's machine. Logging and
    // carrying on instead drops a plugin silently and forever, on a path
    // whose whole job is to not be silent.
    let entries = [
        Registration {
            name: "router",
            kind: PluginKind::Sensor,
        },
        Registration {
            name: "router",
            kind: PluginKind::Channel(LOCAL),
        },
    ];
    build_registry(&entries);
}

#[test]
fn the_production_roster_carries_the_two_sensors_beside_the_four_channels() {
    // The const has to SAY what each entry is, so the sensor rides in the
    // same declaration as the channels rather than in a second list the
    // composition root has to remember to register. The sensor is first
    // because it holds no delivery order to state, which also means a plan
    // that dropped an entry by POSITION rather than by kind would shift
    // every channel after it.
    let declared: Vec<(&str, bool)> = super::ROSTER
        .iter()
        .map(|entry| (entry.name, entry.kind == PluginKind::Sensor))
        .collect();
    assert_eq!(
        declared,
        vec![
            ("router", true),
            ("presence", true),
            ("mobile", false),
            ("macos-banner", false),
            ("hermes", false),
            ("hue", false),
        ]
    );
    assert_eq!(
        super::roster().names(),
        vec![
            "router",
            "presence",
            "mobile",
            "macos-banner",
            "hermes",
            "hue"
        ]
    );
}

#[test]
fn all_selects_every_registration_which_is_what_the_census_reports_against() {
    let selection = three_plugin_registry().all();
    let names: Vec<&str> = selection.iter().map(|r| r.name).collect();
    assert_eq!(names, vec!["mobile", "hermes", "macos-banner"]);
}
