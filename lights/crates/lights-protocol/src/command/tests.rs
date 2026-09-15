use super::*;
fn decode(args: &[&str]) -> Result<Request, String> {
    parse(&args.iter().map(|s| (*s).into()).collect::<Vec<_>>())
}
#[test]
fn implemented_commands_decode_without_settings() {
    for (args, expected) in [
        (vec![], Command::Toggle),
        (vec!["toggle"], Command::Toggle),
        (vec!["on"], Command::On),
        (vec!["off"], Command::Off),
        (vec!["status"], Command::Status),
        (
            vec!["scene", "CC Halo Amber"],
            Command::Scene("CC Halo Amber".into()),
        ),
        (
            vec!["brightness", "up"],
            Command::Brightness(BrightnessRequest::Up),
        ),
        (
            vec!["brightness", "down"],
            Command::Brightness(BrightnessRequest::Down),
        ),
        (
            vec!["brightness", "101"],
            Command::Brightness(BrightnessRequest::Absolute(101)),
        ),
    ] {
        assert_eq!(decode(&args).unwrap().command, expected);
    }
}
#[test]
fn room_option_works_before_and_after_command() {
    for args in [
        ["--room", "Custom Room", "toggle"],
        ["toggle", "--room", "Custom Room"],
    ] {
        assert_eq!(
            decode(&args).unwrap(),
            Request {
                command: Command::Toggle,
                room: Some("Custom Room".into()),
                all: false,
                notify: false,
                over: None
            }
        );
    }
}
#[test]
fn brightness_overflow_is_usage_error() {
    assert!(decode(&["brightness", "18446744073709551616"]).is_err());
    assert_eq!(
        decode(&["brightness", "18446744073709551615"])
            .unwrap()
            .command,
        Command::Brightness(BrightnessRequest::Absolute(u64::MAX))
    );
}
#[test]
fn invalid_brightness_text_is_usage_error() {
    for value in ["-1", "+1", "1.2", "UP", "", "1e2"] {
        assert!(decode(&["brightness", value]).is_err());
    }
    assert_eq!(
        decode(&["brightness", "0"]).unwrap().command,
        Command::Brightness(BrightnessRequest::Absolute(0))
    );
}
#[test]
fn malformed_arguments_never_become_help() {
    for args in [
        vec!["toggle", "extra"],
        vec!["--room"],
        vec!["--room", "--help"],
        vec!["--help", "--typo"],
        vec!["scene"],
        vec!["--room", "x", "--room", "y"],
        vec!["on", "off"],
    ] {
        assert!(decode(&args).is_err());
    }
    assert_eq!(decode(&["--help"]).unwrap().command, Command::Help);
}
#[test]
fn preset_decodes_with_and_without_a_name() {
    assert_eq!(decode(&["preset"]).unwrap().command, Command::Preset(None));
    assert_eq!(
        decode(&["preset", "morning"]).unwrap().command,
        Command::Preset(Some("morning".into()))
    );
}
#[test]
fn preset_refuses_a_room_override_rather_than_ignoring_it() {
    // A preset names its own rooms, so `--room` could only be silently
    // discarded.
    for args in [
        vec!["--room", "studio", "preset", "morning"],
        vec!["preset", "--room", "studio"],
    ] {
        assert!(decode(&args).is_err());
    }
}
#[test]
fn preset_now_decodes_to_the_clock_rather_than_a_preset_of_that_name() {
    assert_eq!(
        decode(&["preset", "now"]).unwrap().command,
        Command::PresetNow
    );
    assert!(decode(&["--room", "studio", "preset", "now"]).is_err());
}
#[test]
fn all_decodes_for_the_two_commands_it_applies_to() {
    for args in [
        vec!["--all", "scene", "Read"],
        vec!["scene", "Read", "--all"],
        vec!["--all", "brightness", "up"],
        vec!["brightness", "50", "--all"],
    ] {
        assert!(decode(&args).unwrap().all);
    }
}
#[test]
fn all_and_room_are_refused_by_name_rather_than_one_winning() {
    for args in [
        vec!["--all", "--room", "studio", "scene", "Read"],
        vec!["scene", "Read", "--room", "studio", "--all"],
    ] {
        let message = decode(&args).unwrap_err();
        assert!(
            message.contains("--all") && message.contains("--room"),
            "{message}"
        );
    }
    assert!(decode(&["--all", "--all", "scene", "Read"]).is_err());
}
#[test]
fn all_is_refused_on_commands_that_do_not_take_it() {
    for args in [
        vec!["--all"],
        vec!["--all", "toggle"],
        vec!["--all", "on"],
        vec!["--all", "off"],
        vec!["--all", "status"],
        vec!["--all", "preset", "evening"],
        vec!["--all", "preset", "now"],
        vec!["--all", "--help"],
    ] {
        assert!(decode(&args).is_err(), "{args:?}");
    }
}
#[test]
fn over_accepts_ms_s_and_m_up_to_the_ceiling() {
    for (spelling, millis) in [
        ("0ms", 0),
        ("750ms", 750),
        ("2s", 2000),
        ("5m", 300_000),
        ("60m", MAX_OVER_MILLIS),
        ("3600000ms", MAX_OVER_MILLIS),
    ] {
        assert_eq!(
            decode(&["--over", spelling, "brightness", "40"])
                .unwrap()
                .over,
            Some(millis),
            "{spelling}"
        );
    }
}
#[test]
fn over_refuses_a_missing_unit_a_bad_unit_and_a_value_past_the_ceiling() {
    for spelling in ["2", "1h", "2sec", "1.5s", "ms", "3600001ms", "4294967296ms"] {
        assert!(
            decode(&["--over", spelling, "brightness", "40"]).is_err(),
            "{spelling}"
        );
    }
}
#[test]
fn duplicate_over_is_a_usage_error() {
    assert!(decode(&["--over", "2s", "--over", "3s", "brightness", "40"]).is_err());
}
