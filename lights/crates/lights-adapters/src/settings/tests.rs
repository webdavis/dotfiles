use super::*;

const VALID: &str = "[controller]\ntype = 'hue'\naddress = '192.0.2.1'\nkey = 'synthetic-secret'\n";

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
            ("bedroom", "3F - Master Bedroom"),
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
