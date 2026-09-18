use super::*;
use lights_domain::{PresetStep, PresetTarget};

const VALID: &str = "[controller]\ntype = 'hue'\naddress = '192.0.2.1'\nkey = 'synthetic-secret'\n\
certificate = 'sha256:0000000000000000000000000000000000000000000000000000000000000000'\n";

#[test]
fn missing_config_returns_config_error() {
    assert!(load(&std::env::temp_dir().join("missing-lights.toml")).is_err());
}
#[test]
fn malformed_config_returns_config_error() {
    assert!(parse("[controller").is_err());
}
#[test]
fn unknown_config_key_is_named() {
    for input in [format!("typo = 1\n{VALID}"), format!("{VALID}typo = 1\n")] {
        assert!(parse(&input).err().unwrap().0.contains("typo"));
    }
}
#[test]
fn missing_controller_type_is_rejected() {
    assert!(parse(&VALID.replace("type = 'hue'\n", "")).is_err());
}
#[test]
fn unknown_controller_type_is_rejected() {
    assert!(
        parse(&VALID.replace("'hue'", "'other'"))
            .err()
            .unwrap()
            .0
            .contains("other")
    );
}
#[test]
fn missing_bridge_address_or_key_is_rejected() {
    for name in ["address", "key"] {
        for value in [None, Some("''"), Some("'  '")] {
            let text = VALID
                .lines()
                .filter(|line| !line.starts_with(name))
                .collect::<Vec<_>>()
                .join("\n");
            let text = if let Some(value) = value {
                format!("{text}\n{name} = {value}")
            } else {
                text
            };
            assert!(parse(&text).is_err(), "accepted {name}");
        }
    }
}
#[test]
fn config_error_does_not_print_key() {
    for text in [
        format!("{VALID}typo = 1"),
        format!("{VALID}broken = 'synthetic-secret"),
    ] {
        let error = parse(&text).err().unwrap();
        assert!(!error.0.contains("synthetic-secret"));
        assert!(!error.0.contains('\n'));
    }
}
#[test]
fn brightness_step_defaults_to_fifteen() {
    assert_eq!(parse(VALID).unwrap().step, 15);
}
#[test]
fn configured_step_is_used() {
    for step in [1, 5, 15, 100] {
        assert_eq!(
            parse(&format!("{VALID}[brightness]\nstep={step}"))
                .unwrap()
                .step,
            step
        );
    }
}
#[test]
fn step_outside_1_through_100_is_rejected() {
    for step in [0, 101, -1] {
        assert!(parse(&format!("{VALID}[brightness]\nstep={step}")).is_err());
    }
    assert_eq!(
        parse(&format!("{VALID}[brightness]\nstep=1")).unwrap().step,
        1
    );
}
#[test]
fn shipped_defaults_resolve_studio_and_four_scene_rotation() {
    for text in [VALID, include_str!("../../tests/fixtures/defaults.toml")] {
        let settings = parse(text).unwrap();
        assert_eq!(settings.default_room.as_str(), "3F - Studio");
        for (alias, room) in [
            ("studio", "3F - Studio"),
            ("bedroom", "3F - MBedroom"),
            ("kitchen", "2F - Kitchen"),
        ] {
            assert_eq!(settings.aliases.resolve(alias).unwrap().as_str(), room);
        }
        let rotation = &settings.rotation;
        for (current, next) in [
            ("Dimmed", "Read"),
            ("Read", "Energize"),
            ("Energize", "Concentrate"),
            ("Concentrate", "Dimmed"),
            ("CC Halo Amber", "Read"),
            ("CC Halo Daylight", "Read"),
        ] {
            assert_eq!(rotation.next(Some(current)), next);
        }
    }
}
#[test]
fn configured_room_alias_and_rotation_are_used() {
    let settings = parse(&format!("default_room='Office'\n{VALID}[rooms]\nwork='Office'\n[scenes]\nrotation=['A','B']\nfallback='B'")).unwrap();
    assert_eq!(settings.default_room.as_str(), "Office");
    assert_eq!(settings.aliases.resolve("work").unwrap().as_str(), "Office");
    assert_eq!(settings.rotation.next(Some("A")), "B");
    assert_eq!(settings.rotation.previous(Some("A")), "B");
}
#[test]
fn bedroom_alias_resolves_to_the_bridge_room_name() {
    // The bridge inventory read on 2026-09-14 names this room `3F - MBedroom`.
    // The earlier `3F - Master Bedroom` matched no room, so `--room bedroom`
    // exited 2 for every command.
    for text in [VALID, include_str!("../../tests/fixtures/defaults.toml")] {
        assert_eq!(
            parse(text)
                .unwrap()
                .aliases
                .resolve("bedroom")
                .unwrap()
                .as_str(),
            "3F - MBedroom"
        );
    }
}
#[test]
fn preset_table_parses_ordered_scene_and_off_steps_through_aliases() {
    let settings = parse(&format!(
        "{VALID}[rooms]\nwork = 'Office'\n\
         [presets]\n\
         evening = [{{ room = 'work', scene = 'Read' }}, {{ room = 'Hallway', off = true }}]\n"
    ))
    .unwrap();
    assert_eq!(settings.presets.names().collect::<Vec<_>>(), ["evening"]);
    assert_eq!(
        settings.presets.plan("evening"),
        Some(
            &[
                PresetStep {
                    room: RoomName::new("Office").unwrap(),
                    target: PresetTarget::Scene("Read".into())
                },
                PresetStep {
                    room: RoomName::new("Hallway").unwrap(),
                    target: PresetTarget::Off
                },
            ][..]
        )
    );
}
#[test]
fn no_preset_table_configures_no_presets() {
    assert_eq!(parse(VALID).unwrap().presets.names().count(), 0);
}
#[test]
fn a_preset_step_naming_neither_or_both_targets_is_rejected() {
    for step in [
        "{ room = 'Office' }",
        "{ room = 'Office', scene = 'Read', off = true }",
        "{ room = 'Office', off = false }",
        "{ room = 'Office', scene = '' }",
        "{ scene = 'Read' }",
        "{ room = 'Office', scene = 'Read', typo = 1 }",
    ] {
        assert!(
            parse(&format!("{VALID}[presets]\nevening = [{step}]\n")).is_err(),
            "accepted {step}"
        );
    }
    for preset in [
        "evening = []",
        "evening = 'Read'",
        "'' = [{ room = 'A', off = true }]",
    ] {
        assert!(
            parse(&format!("{VALID}[presets]\n{preset}\n")).is_err(),
            "accepted {preset}"
        );
    }
}
#[test]
fn shipped_presets_name_three_times_of_day_over_the_three_aliased_rooms() {
    let settings = parse(include_str!("../../tests/fixtures/defaults.toml")).unwrap();
    assert_eq!(
        settings.presets.names().collect::<Vec<_>>(),
        ["afternoon", "evening", "morning"]
    );
    for (preset, scene) in [
        ("morning", "Energize"),
        ("afternoon", "Concentrate"),
        ("evening", "Read"),
    ] {
        assert_eq!(
            settings.presets.plan(preset),
            Some(
                &["3F - Studio", "3F - MBedroom", "2F - Kitchen"].map(|room| PresetStep {
                    room: RoomName::new(room).unwrap(),
                    target: PresetTarget::Scene(scene.into()),
                })[..]
            )
        );
    }
}
#[test]
fn rotation_memory_is_off_unless_the_config_asks_for_it() {
    for text in [VALID, include_str!("../../tests/fixtures/defaults.toml")] {
        assert!(!parse(text).unwrap().remember_position);
    }
    assert!(
        parse(&format!("{VALID}[scenes]\nremember_position = true"))
            .unwrap()
            .remember_position
    );
}
#[test]
fn a_remember_position_that_is_not_a_boolean_is_named_and_rejected() {
    for value in ["'true'", "1", "[]"] {
        let error = parse(&format!("{VALID}[scenes]\nremember_position = {value}"))
            .err()
            .unwrap();
        assert!(error.0.contains("remember_position"), "accepted {value}");
    }
}
const WINDOW_PRESETS: &str = "[presets]\nbed = [{ room = 'studio', off = true }]\n";

#[test]
fn preset_windows_are_read_as_minutes_of_the_local_day() {
    let settings = parse(&format!(
        "{VALID}{WINDOW_PRESETS}\
         [[preset_windows]]\nstart = '22:00'\nend = '06:30'\npreset = 'bed'\n"
    ))
    .unwrap();
    assert_eq!(settings.preset_windows.preset_at(23 * 60), Some("bed"));
    assert_eq!(settings.preset_windows.preset_at(6 * 60 + 30), None);
}
#[test]
fn a_window_naming_an_unknown_preset_is_refused_at_load() {
    let error = parse(&format!(
        "{VALID}{WINDOW_PRESETS}\
         [[preset_windows]]\nstart = '22:00'\nend = '06:00'\npreset = 'absent'\n"
    ))
    .err()
    .unwrap();
    assert!(error.0.contains("absent"), "{}", error.0);
}
#[test]
fn a_window_time_that_is_not_hh_colon_mm_is_refused() {
    for time in [
        "'6:00'", "'24:00'", "'22:60'", "'2200'", "'noon'", "'+3:00'", "6", "[]",
    ] {
        let input = format!(
            "{VALID}{WINDOW_PRESETS}\
             [[preset_windows]]\nstart = {time}\nend = '06:00'\npreset = 'bed'\n"
        );
        assert!(parse(&input).is_err(), "accepted {time}");
    }
}
#[test]
fn preset_windows_that_are_not_a_list_of_tables_are_refused() {
    for table in ["preset_windows = 1", "preset_windows = []"] {
        assert!(parse(&format!("{VALID}{WINDOW_PRESETS}{table}\n")).is_err());
    }
}
#[test]
fn a_controller_without_a_certificate_is_refused_and_names_the_enrolling_command() {
    let text = VALID
        .lines()
        .filter(|line| !line.starts_with("certificate"))
        .collect::<Vec<_>>()
        .join("\n");
    let Err(refusal) = parse(&text) else {
        panic!("accepted a config it should refuse");
    };
    let refusal = refusal.0;
    assert!(refusal.contains("certificate"), "{refusal}");
    assert!(refusal.contains("lights enroll"), "{refusal}");
}
#[test]
fn a_malformed_certificate_is_refused_without_echoing_it() {
    for value in ["''", "'nonsense'", "'sha256:abc'", "'sha1:00'"] {
        let text = VALID.replace(
            "certificate = 'sha256:0000000000000000000000000000000000000000000000000000000000000000'",
            &format!("certificate = {value}"),
        );
        let Err(refusal) = parse(&text) else {
            panic!("accepted a config it should refuse");
        };
        let refusal = refusal.0;
        assert!(!refusal.contains("nonsense"), "{refusal}");
    }
}
#[test]
fn the_endpoint_alone_reads_without_a_certificate_so_enrolling_is_possible_before_one_exists() {
    let text = VALID
        .lines()
        .filter(|line| !line.starts_with("certificate"))
        .collect::<Vec<_>>()
        .join("\n");
    let Ok(endpoint) = endpoint(&text) else {
        panic!("refused an endpoint with no certificate");
    };
    assert_eq!(endpoint.address, "192.0.2.1");
    assert_eq!(endpoint.timeout_secs, 2);
    assert!(parse(&text).is_err(), "the full parse still refuses it");
}
#[test]
fn the_endpoint_refuses_the_same_bad_address_type_and_timeout_the_full_parse_does() {
    for text in [
        VALID.replace("'hue'", "'other'"),
        VALID.replace("192.0.2.1", "bridge/../etc"),
        format!("{VALID}timeout_secs = 0\n"),
    ] {
        assert!(endpoint(&text).is_err(), "{text}");
    }
}
