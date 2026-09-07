#[test]
fn the_doctors_own_wording_names_only_keys_the_router_table_serves() {
    // THE THIRD DOCUMENT. The template says what to write, the refusals say
    // what is wrong with what was written, and the doctor's setup report
    // says which key to go and set; a key renamed in two of the three is an
    // operator sent to a spelling nothing reads.
    //
    // READ OFF THE REPORT ITSELF, never restated here. A test that asserts
    // string literals against the roster's string literals agrees with
    // itself whatever the doctor actually says, which is the exact drift it
    // is named for: rename `router_url` to `url` in the sentence below and
    // nothing else, and a test written that way stays green.
    use crate::home::{DeviceKey, SetupFailure, setup_report};
    let serves = super::keys_of("plugins.router").expect("the router table is in the roster");
    let quoted = "\"x\"".to_string();
    for (failure, sends_the_operator_to_a_key) in [
        (SetupFailure::NoConfigFile, false),
        (SetupFailure::ConfigError("refused".to_string()), false),
        (SetupFailure::NoRouterPlugin, false),
        (SetupFailure::RouterDisabled, true),
        (SetupFailure::NoType, true),
        (SetupFailure::UnknownType("asus".to_string()), true),
        (SetupFailure::InvalidRouterTable, true),
        (SetupFailure::NoDeviceIdentifier, true),
        (
            SetupFailure::InvalidDeviceKey {
                key: DeviceKey::Mac,
                found: quoted.clone(),
            },
            true,
        ),
        (
            SetupFailure::InvalidDeviceKey {
                key: DeviceKey::Hostname,
                found: quoted.clone(),
            },
            true,
        ),
        (
            SetupFailure::InvalidDeviceKey {
                key: DeviceKey::Ipv4,
                found: quoted.clone(),
            },
            true,
        ),
        (SetupFailure::NoApiKey, true),
    ] {
        let said = setup_report(&failure);
        let words: Vec<&str> = said
            .split(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'))
            .collect();
        // A WORD CARRYING AN UNDERSCORE IS A CONFIG KEY and nothing else in
        // this vocabulary: the prose around it is English. So one that the
        // table does not serve is a key renamed in the code and left
        // standing here.
        for word in words.iter().filter(|word| word.contains('_')) {
            assert!(
                serves.contains(word),
                "the report says `{word}`, which the router table does not serve: {said}"
            );
        }
        // AND THE OTHER DIRECTION, which is the half a spelling check
        // cannot see: a line that is supposed to send the operator to a key
        // has to still name one, or `router_url` became `url` and the
        // sentence now points at nothing.
        assert_eq!(
            words.iter().any(|word| serves.contains(word)),
            sends_the_operator_to_a_key,
            "whether this line names a key the table serves changed: {said}"
        );
    }
}
