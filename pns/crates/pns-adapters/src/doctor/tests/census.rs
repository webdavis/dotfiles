use super::*;

// --- the census ----------------------------------------------------------

#[test]
fn the_check_list_holds_one_entry_per_registration_in_registration_order() {
    // WITH NOTHING ENABLED, so a census that walked the SELECTION would
    // return an empty report and lose every plugin at once. Registration
    // order is delivery order, and the report is read against the config.
    let (registry, registered, selected) = census("");
    assert_eq!(
        checks(&registered, &selected, ConfigState::Read)
            .iter()
            .map(|check| check.plugin)
            .collect::<Vec<_>>(),
        registry.names(),
        "a report cannot silently omit a plugin"
    );
}

#[test]
fn a_registered_plugin_the_config_did_not_enable_is_a_skip_that_says_which() {
    // BOTH WAYS a config declines a plugin: never naming it, and naming it
    // switched off. Neither is an error and both have to be visible, or
    // the operator reads a short report as a complete one.
    assert_eq!(
        kind_for("[plugins.hermes]\nenabled = true\n", "mobile"),
        CheckKind::Skipped(NOT_ENABLED)
    );
    assert_eq!(
        kind_for("[plugins.mobile]\nenabled = false\n", "mobile"),
        CheckKind::Skipped(NOT_ENABLED)
    );
}

#[test]
fn a_plugin_the_selection_left_out_is_skipped_in_words_true_of_this_machine() {
    // THREE STATES, THREE EDITS. "Not enabled in the config" is a lie on a
    // machine with no config: it points the operator at a file that does
    // not exist, and it became the ORDINARY report there the moment the
    // fallback narrowed from the whole roster to the core. The unreadable
    // config is its own state again, because one is fixed by writing a
    // file and the other by repairing one.
    let registry = roster();
    let registered = registry.all();
    let core = registry.core();
    let reason = |config| {
        checks(&registered, &core, config)
            .into_iter()
            .find(|check| check.plugin == "hermes")
            .expect("hermes is registered")
            .kind
    };
    assert_eq!(reason(ConfigState::Read), CheckKind::Skipped(NOT_ENABLED));
    assert_eq!(reason(ConfigState::Absent), CheckKind::Skipped(NO_CONFIG));
    assert_eq!(
        reason(ConfigState::Unreadable),
        CheckKind::Skipped(UNREADABLE_CONFIG)
    );
    // AND THE THREE ARE DIFFERENT SENTENCES, which is the whole point: a
    // constant accidentally pointed at another would pass every equality
    // above and report one state as another.
    assert_eq!(
        [NOT_ENABLED, NO_CONFIG, UNREADABLE_CONFIG]
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        3
    );
}

#[test]
fn a_selected_sensor_is_a_skip_because_no_leg_can_ever_reach_one() {
    assert_eq!(
        kind_for("[plugins.router]\nenabled = true\n", "router"),
        CheckKind::Skipped(A_SENSOR)
    );
}

#[test]
fn a_selected_channel_no_event_dispatches_is_a_pulse_rather_than_a_send() {
    assert_eq!(
        kind_for("[plugins.hue]\nenabled = true\n", "hue"),
        CheckKind::Pulse
    );
}

#[test]
fn a_selected_event_dispatched_channel_is_a_send() {
    for plugin in ["mobile", "macos-banner", "hermes"] {
        assert_eq!(
            kind_for(
                "[plugins.mobile]\nenabled = true\n[plugins.macos-banner]\nenabled = true\n\
                     [plugins.hermes]\nenabled = true\n",
                plugin
            ),
            CheckKind::Send,
            "plugin: {plugin}"
        );
    }
}
