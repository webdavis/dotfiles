//! The plan's own tests, kept beside the config edge they drive: every
//! selection here comes out of a real registry and a REAL PARSED CONFIG on
//! purpose, so each plan test exercises registration, selection and the plan
//! end to end. The policy itself lives in `pns-domain`.

use super::{Leg, ReportMode, channel_plan};
use pns_adapters::parse_config;
use pns_domain::registry::{Registry, Routing, Selection};

mod scope;

/// A leg the operator is SHOWN something by: the phone card or this
/// machine's banner. Stated at the call site rather than derived, so a
/// plan that mislabelled one fails the test that names it.
fn decorative(name: &'static str, mode: ReportMode) -> Leg {
    Leg {
        name,
        mode,
        decorative: true,
    }
}

/// A leg that only records: the durable log, which every event reaches
/// whether or not anyone is there to see it.
fn logged(name: &'static str, mode: ReportMode) -> Leg {
    Leg {
        name,
        mode,
        decorative: false,
    }
}

/// A plan that reaches every surface, so a narrowing test varies one
/// thing: the flags. `card` is the phone, `banner` this machine's screen.
fn reaching(banner: bool, card: bool) -> pns_domain::surface::DeliveryPlan {
    pns_domain::surface::DeliveryPlan {
        banner,
        phone_card: card,
        pulse: false,
    }
}

/// Every selection comes out of a real registry and a real config, so
/// each plan test exercises register_channel, enabled and channel_plan END
/// TO END: a registry that mislaid a routing declaration fails these, not
/// only its own unit tests.
fn select(registry: &Registry, config_text: &str) -> Selection {
    registry
        .enabled(&parse_config(config_text).unwrap().plugin_switches())
        .unwrap()
}

const ALL_THREE_ON: &str = "[plugins.mobile]\nenabled = true\n[plugins.hermes]\nenabled = true\n[plugins.macos-banner]\nenabled = true\n";

fn three_enabled() -> Selection {
    select(&pns_domain::registry::roster(), ALL_THREE_ON)
}

const SENSOR_AND_THREE_ON: &str = "[plugins.router]\nenabled = true\n[plugins.mobile]\nenabled = true\n[plugins.hermes]\nenabled = true\n[plugins.macos-banner]\nenabled = true\n";

/// A selection holding an enabled sensor AND the three enabled channels,
/// so every sensor assertion carries its own positive control. The real
/// roster declares the sensor FIRST, so a plan that dropped an entry by
/// POSITION rather than by kind would shift every channel after it and
/// fail these outright.
fn sensor_and_three_enabled() -> Selection {
    let enabled = select(&pns_domain::registry::roster(), SENSOR_AND_THREE_ON);
    assert!(
        enabled.iter().any(|entry| entry.name == "router"),
        "the sensor must be SELECTED, or these test a selection miss rather than a plan filter"
    );
    enabled
}

// --- channel_plan ------------------------------------------------------

#[test]
fn the_alert_path_plans_phone_then_banner_then_log() {
    // The two presence-sensitive surfaces come first and the durable log
    // last: the plan is computed from one reading of where the operator
    // is, and hermes posts over the network under its own deadline.
    assert_eq!(
        channel_plan(
            &three_enabled(),
            pns_domain::DeliveryScope::Automatic,
            reaching(true, true)
        ),
        vec![
            decorative("mobile", ReportMode::Silent),
            decorative("macos-banner", ReportMode::Silent),
            logged("hermes", ReportMode::Silent),
        ]
    );
}

#[test]
fn a_selected_sensor_is_never_a_leg_on_the_alert_path() {
    // A sensor is an INPUT. It is selected, it occupies a config table,
    // and no event routes to it: without this the engine would try to
    // exec `channels/router.sh` on every notification. The three channels
    // are the positive control, so a plan that dropped everything cannot
    // pass by looking like a suppressed sensor.
    assert_eq!(
        channel_plan(
            &sensor_and_three_enabled(),
            pns_domain::DeliveryScope::Automatic,
            reaching(true, true)
        ),
        vec![
            decorative("mobile", ReportMode::Silent),
            decorative("macos-banner", ReportMode::Silent),
            logged("hermes", ReportMode::Silent),
        ]
    );
}

#[test]
fn a_suppressed_phone_drops_only_the_presence_gated_leg() {
    assert_eq!(
        channel_plan(
            &three_enabled(),
            pns_domain::DeliveryScope::Automatic,
            reaching(true, false)
        ),
        vec![
            decorative("macos-banner", ReportMode::Silent),
            logged("hermes", ReportMode::Silent)
        ]
    );
}

#[test]
fn no_enabled_plugins_plan_nothing_under_every_flag() {
    // An unconfigured machine has an empty plan, not a crash and not a
    // built-in fallback: the caller reports the empty verdict.
    let none = select(&pns_domain::registry::roster(), "");
    for (scope, phone) in [
        (pns_domain::DeliveryScope::Automatic, true),
        (pns_domain::DeliveryScope::Automatic, false),
        (pns_domain::DeliveryScope::LocalOnly, true),
        (pns_domain::DeliveryScope::RemoteOnly, true),
    ] {
        assert_eq!(channel_plan(&none, scope, reaching(true, phone)), vec![]);
    }
}

#[test]
fn a_plugin_that_is_not_event_dispatched_is_never_a_leg_however_it_is_selected() {
    // hue is registered and selectable, and the pulse mode runs it, but no
    // notification may route to it. Without this the roster's last entry
    // would start appearing as a channel on every event.
    let enabled = select(
        &pns_domain::registry::roster(),
        "[plugins.hue]\nenabled = true\n[plugins.hermes]\nenabled = true\n",
    );
    assert_eq!(
        channel_plan(
            &enabled,
            pns_domain::DeliveryScope::Automatic,
            reaching(true, true)
        ),
        vec![logged("hermes", ReportMode::Silent)]
    );
    assert_eq!(
        channel_plan(
            &enabled,
            pns_domain::DeliveryScope::LocalOnly,
            reaching(true, true)
        ),
        Vec::new(),
        "not even the local-only path, which hue would otherwise match"
    );
}

#[test]
fn the_unconfigured_machine_knows_every_sensor_and_still_plans_channels_only() {
    // `all()` is the fallback a MISSING or BROKEN config lands on, so it
    // has to hold the sensors: the unknown-name refusal reads that same
    // roster, and a fallback that forgot them would call a correctly
    // spelled sensor a typo. It selects them WITHOUT planning them, which
    // is the pair that matters: the machine nobody configured gets the
    // channels it always got and no exec attempt named after a sensor.
    let all = pns_domain::registry::roster().all();
    assert!(
        all.iter().any(|entry| entry.name == "router"),
        "the fallback roster must know the sensor's name"
    );
    assert_eq!(
        channel_plan(
            &all,
            pns_domain::DeliveryScope::Automatic,
            reaching(true, true)
        ),
        vec![
            decorative("mobile", ReportMode::Silent),
            decorative("macos-banner", ReportMode::Silent),
            logged("hermes", ReportMode::Silent),
        ]
    );
}

#[test]
fn the_presence_gate_means_one_thing_under_every_flag() {
    // Two hypotheticals pin the gate as a COMPOSED filter rather than a
    // branch: a presence-gated LOCAL plugin (a wearable's buzz) on the
    // local-only path, and a presence-gated DURABLE plugin (a phone
    // pager log) on the remote-only path. An implementation that skips
    // the gate inside either flag's branch keeps one of them wrongly.
    let mut registry = Registry::new();
    registry
        .register_channel(
            "buzz",
            Routing {
                local: true,
                presence_gated: true,
                durable: false,
                event_dispatched: true,
            },
        )
        .unwrap();
    registry
        .register_channel(
            "pager",
            Routing {
                local: false,
                presence_gated: true,
                durable: true,
                event_dispatched: true,
            },
        )
        .unwrap();
    let both = "[plugins.buzz]\nenabled = true\n[plugins.pager]\nenabled = true\n";
    let enabled = select(&registry, both);

    assert_eq!(
        channel_plan(
            &enabled,
            pns_domain::DeliveryScope::LocalOnly,
            reaching(true, true)
        ),
        vec![decorative("buzz", ReportMode::Silent)]
    );
    assert_eq!(
        channel_plan(
            &enabled,
            pns_domain::DeliveryScope::LocalOnly,
            reaching(true, false)
        ),
        vec![]
    );
    assert_eq!(
        channel_plan(
            &enabled,
            pns_domain::DeliveryScope::RemoteOnly,
            reaching(true, true)
        ),
        vec![decorative("pager", ReportMode::ReportOutcome)]
    );
    assert_eq!(
        channel_plan(
            &enabled,
            pns_domain::DeliveryScope::RemoteOnly,
            reaching(true, false)
        ),
        vec![]
    );
}

#[test]
fn a_mode_names_what_the_channel_contract_spells_in_the_event() {
    assert_eq!(ReportMode::Silent.as_str(), "async");
    assert_eq!(ReportMode::ReportOutcome.as_str(), "sync");
}
