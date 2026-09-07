use super::*;

#[test]
fn a_machine_with_no_config_runs_the_core_and_nothing_that_needs_arming() {
    // THE FALLBACK IS THE CORE, not the whole roster (operator ruling
    // 2026-08-31). A machine with no config gets the two destinations that
    // are useful the moment the binary lands; hue, hermes and router each
    // need a bridge, a route or an API key stood up before they can do
    // anything at all, so defaulting them on delivers nothing and reports
    // three failures.
    use crate::config::LoadOutcome;
    let (selection, warning) = select_plugins(&roster(), Ok(LoadOutcome::Missing));
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
        selection_names(&roster().core()),
        vec!["mobile", "macos-banner"],
        "every core name is a registered plugin"
    );
    // AND IT IS ONLY A FALLBACK. A config that exists says what runs, so
    // writing one that omits a core plugin turns that plugin off; nothing
    // is quietly always-on.
    let config = parse_config("[plugins.hermes]\nenabled = true\n").unwrap();
    let selection = roster().enabled(&config.plugin_switches()).unwrap();
    assert_eq!(selection_names(&selection), vec!["hermes"]);
}

#[test]
fn a_loaded_config_is_authoritative() {
    use crate::config::LoadOutcome;
    let config = parse_config("[plugins.hermes]\nenabled = true\n").unwrap();
    let (selection, warning) = select_plugins(&roster(), Ok(LoadOutcome::Loaded(config)));
    assert_eq!(selection_names(&selection), vec!["hermes"]);
    assert_eq!(warning, None);
}

#[test]
fn a_broken_config_is_loud_but_never_turns_notifications_off() {
    // THE CORE, not the whole roster: the three left out keep their
    // credentials in the very file nobody could read, so running them
    // would report three failures about a config error already on stderr.
    use crate::config::ConfigError;
    let (selection, warning) = select_plugins(
        &roster(),
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
    let (selection, warning) = select_plugins(&roster(), Ok(LoadOutcome::Loaded(config)));
    assert_eq!(
        selection_names(&selection),
        roster().names(),
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
    let (selection, warning) = select_plugins(&roster(), Ok(LoadOutcome::Loaded(config)));
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
        roster().enabled(&old.plugin_switches()),
        Err(RegistryError::UnknownPlugin("moshi".to_string()))
    );
    let (_, warning) = select_plugins(&roster(), Ok(LoadOutcome::Loaded(old)));
    assert!(
        warning
            .expect("the retired name is said aloud")
            .contains("moshi"),
        "the operator is told which name stopped working"
    );

    let new = parse_config("[plugins.mobile]\nenabled = true\n").unwrap();
    let selection = roster().enabled(&new.plugin_switches()).unwrap();
    assert_eq!(selection_names(&selection), vec!["mobile"]);
}

#[test]
fn a_true_typo_is_still_refused() {
    use crate::config::LoadOutcome;
    let config = parse_config("[plugins.mosih]\nenabled = true\n").unwrap();
    let (_, warning) = select_plugins(&roster(), Ok(LoadOutcome::Loaded(config)));
    assert!(
        warning
            .expect("the typo is still the defect")
            .contains("mosih")
    );
}
