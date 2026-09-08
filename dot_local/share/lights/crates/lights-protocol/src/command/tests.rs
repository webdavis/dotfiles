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
                notify: false
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
