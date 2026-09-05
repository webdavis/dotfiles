//! Selection's own tests, kept here because every one of them builds a REAL
//! PARSED CONFIG: what a config selects, what it refuses, and which plugins a
//! machine runs when the file is missing, unreadable or names a typo.
//!
//! The registry tests that need no config moved to `pns-domain` with the
//! registry itself.

use super::Selection;
use super::{Registry, RegistryError, Routing};
use crate::config::parse_config;

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

// --- plugin kinds -------------------------------------------------------

#[test]
fn a_sensor_registers_by_name_so_a_typo_near_it_is_still_refused() {
    // A sensor carries no routing, but it occupies a config table like any
    // channel, so the registry has to know its name or the unknown-name
    // refusal would call the operator's correct spelling a typo.
    let mut registry = Registry::new();
    registry.register_channel("hermes", DURABLE).unwrap();
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
        super::roster().enabled(&config.plugin_switches()),
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
        super::roster().enabled(&config.plugin_switches()),
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
    let names: Vec<&str> = super::roster()
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
    let enabled = three_plugin_registry()
        .enabled(&config.plugin_switches())
        .unwrap();
    let names: Vec<&str> = enabled.iter().map(|r| r.name).collect();
    assert_eq!(names, vec!["mobile", "macos-banner"]);
}

#[test]
fn a_disabled_or_omitted_plugin_is_simply_not_selected() {
    let config =
        parse_config("[plugins.mobile]\nenabled = true\n[plugins.hermes]\nenabled = false\n")
            .unwrap();
    let enabled = three_plugin_registry()
        .enabled(&config.plugin_switches())
        .unwrap();
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
        three_plugin_registry().enabled(&config.plugin_switches()),
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
        three_plugin_registry().enabled(&config.plugin_switches()),
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
    registry.register_channel("hermes", DURABLE).unwrap();
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
    let enabled = three_plugin_registry()
        .enabled(&config.plugin_switches())
        .unwrap();
    assert!(enabled.is_empty());
}

// --- plugin selection at the composition root ---------------------------

fn selection_names(selection: &Selection) -> Vec<&str> {
    selection.iter().map(|r| r.name).collect()
}

#[test]
fn a_machine_with_no_config_runs_the_core_and_nothing_that_needs_arming() {
    // THE FALLBACK IS THE CORE, not the whole roster (operator ruling
    // 2026-08-31). A machine with no config gets the two destinations that
    // are useful the moment the binary lands; hue, hermes and router each
    // need a bridge, a route or an API key stood up before they can do
    // anything at all, so defaulting them on delivers nothing and reports
    // three failures.
    use crate::config::LoadOutcome;
    let (selection, warning) = super::select_plugins(&super::roster(), Ok(LoadOutcome::Missing));
    assert_eq!(selection_names(&selection), vec!["mobile", "macos-banner"]);
    assert_eq!(warning, None);
}

#[test]
fn the_core_is_two_registered_plugins_and_the_config_still_beats_it() {
    // THE NAME LIST IS THE DRIFT RISK: a misspelling in `CORE` selects
    // nothing rather than failing to compile, and the machine with no
    // config would go quiet with nothing to look at. So the core is
    // asserted against the REAL roster, both members named.
    assert_eq!(
        selection_names(&super::roster().core()),
        vec!["mobile", "macos-banner"],
        "every core name is a registered plugin"
    );
    // AND IT IS ONLY A FALLBACK. A config that exists says what runs, so
    // writing one that omits a core plugin turns that plugin off; nothing
    // is quietly always-on.
    let config = parse_config("[plugins.hermes]\nenabled = true\n").unwrap();
    let selection = super::roster().enabled(&config.plugin_switches()).unwrap();
    assert_eq!(selection_names(&selection), vec!["hermes"]);
}

#[test]
fn a_loaded_config_is_authoritative() {
    use crate::config::LoadOutcome;
    let config = parse_config("[plugins.hermes]\nenabled = true\n").unwrap();
    let (selection, warning) =
        super::select_plugins(&super::roster(), Ok(LoadOutcome::Loaded(config)));
    assert_eq!(selection_names(&selection), vec!["hermes"]);
    assert_eq!(warning, None);
}

#[test]
fn a_broken_config_is_loud_but_never_turns_notifications_off() {
    // THE CORE, not the whole roster: the three left out keep their
    // credentials in the very file nobody could read, so running them
    // would report three failures about a config error already on stderr.
    use crate::config::ConfigError;
    let (selection, warning) = super::select_plugins(
        &super::roster(),
        Err(ConfigError::Malformed(
            "key with no value at line 1".to_string(),
        )),
    );
    assert_eq!(selection_names(&selection), vec!["mobile", "macos-banner"]);
    let warning = warning.expect("a broken config must be said aloud");
    assert!(warning.contains("key with no value"));
}

#[test]
fn a_config_naming_an_unknown_plugin_is_loud_and_falls_back_to_the_roster() {
    // THE WHOLE ROSTER, and the arm is the reason. This config PARSED, so
    // every credential in it is in hand and the composition root has
    // already read hue's table, hermes's key and the recap off it before
    // selection runs. The core fallback exists for a file nobody could
    // read; applying it here lets one mistyped table name cost a fully
    // configured machine its durable paper trail and its lights, which is
    // a blast radius no ruling asked for.
    use crate::config::LoadOutcome;
    let config =
        parse_config("[plugins.mosih]\nenabled = true\n[plugins.hermes]\nenabled = true\n")
            .unwrap();
    let (selection, warning) =
        super::select_plugins(&super::roster(), Ok(LoadOutcome::Loaded(config)));
    assert_eq!(
        selection_names(&selection),
        super::roster().names(),
        "a config-present machine keeps every plugin; the typo is loud, not fatal"
    );
    let warning = warning.expect("the typo'd name must be said aloud");
    assert!(warning.contains("mosih"));
    assert!(
        warning.contains("running every built-in plugin"),
        "and the line says what still runs: {warning}"
    );
}

#[test]
fn a_hue_table_selects_hue_like_any_other_plugin_and_warns_about_nothing() {
    // It used to be a string exception stripped before the unknown-name
    // refusal. It is a registration now, so configuring the pulse is
    // ordinary and costs the operator no part of their event selection.
    use crate::config::LoadOutcome;
    let config =
        parse_config("[plugins.hermes]\nenabled = true\n[plugins.hue]\nenabled = true\n").unwrap();
    let (selection, warning) =
        super::select_plugins(&super::roster(), Ok(LoadOutcome::Loaded(config)));
    assert_eq!(selection_names(&selection), vec!["hermes", "hue"]);
    assert_eq!(warning, None);
}

#[test]
fn the_old_moshi_table_name_is_refused_and_the_mobile_one_is_served() {
    // THE RENAME HAS NO BACK ROAD, and it needs none: the template is the
    // only pns config anyone has and it regenerates at apply. So
    // `[plugins.moshi]` is not a second spelling of the phone plugin, it
    // is a name nothing registered, and it gets the refusal every typo
    // gets rather than quietly selecting the plugin it used to name.
    use crate::config::LoadOutcome;
    let old = parse_config("[plugins.moshi]\nenabled = true\n").unwrap();
    assert_eq!(
        super::roster().enabled(&old.plugin_switches()),
        Err(RegistryError::UnknownPlugin("moshi".to_string()))
    );
    let (_, warning) = super::select_plugins(&super::roster(), Ok(LoadOutcome::Loaded(old)));
    assert!(
        warning
            .expect("the retired name is said aloud")
            .contains("moshi"),
        "the operator is told which name stopped working"
    );

    let new = parse_config("[plugins.mobile]\nenabled = true\n").unwrap();
    let selection = super::roster().enabled(&new.plugin_switches()).unwrap();
    assert_eq!(selection_names(&selection), vec!["mobile"]);
}

#[test]
fn a_true_typo_is_still_refused() {
    use crate::config::LoadOutcome;
    let config = parse_config("[plugins.mosih]\nenabled = true\n").unwrap();
    let (_, warning) = super::select_plugins(&super::roster(), Ok(LoadOutcome::Loaded(config)));
    assert!(
        warning
            .expect("the typo is still the defect")
            .contains("mosih")
    );
}
