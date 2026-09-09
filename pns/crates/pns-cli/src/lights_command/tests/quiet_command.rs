use super::*;

#[test]
fn a_duration_outside_the_bounds_is_refused_by_what_was_typed() {
    // ONE SPELLING OF "HOW LONG" IN THE WHOLE CRATE. The refusal is
    // `parse_duration`'s own, word for word, because a second wording here
    // would be a second set of bounds the day either one moved.
    let known = places(&["3F - Studio"]);
    for typed in ["0s", "25h", "1441m", "9223372036854775807h"] {
        assert_eq!(
            quiet_command(&typed_at("3F - Studio", typed), &known, ONE_HOUR),
            Err(format!(
                "pns: quiet duration {typed:?} is outside 1s to 24h"
            )),
            "typed: {typed:?}"
        );
    }
    for typed in ["30", "", "1d", " 5m"] {
        assert_eq!(
            quiet_command(&typed_at("3F - Studio", typed), &known, ONE_HOUR),
            Err(format!(
                "pns: quiet duration {typed:?} is not <count><s|m|h>"
            )),
            "typed: {typed:?}"
        );
    }
    assert_eq!(
        quiet_command(&typed_at("3F - Studio", "30m"), &known, ONE_HOUR),
        Ok(QuietCommand::Mute {
            place: "3F - Studio".to_string(),
            seconds: 1_800,
        }),
        "and the two ends of the range are what the bounds let through"
    );
}

#[test]
fn a_place_the_config_does_not_name_is_refused_rather_than_silently_stored() {
    // A MUTE IS A LINE NOTHING WILL EVER MATCH. Stored quietly, the lamp
    // the operator meant to quiet goes on flashing while the command
    // reports success, and the only evidence they get is the lamp itself at
    // the hour they were trying not to be disturbed.
    let known = places(&["3F - Studio", "3F - Studio - HCL3"]);
    assert_eq!(
        quiet_command(&typed_at("3F - Nowhere", "30m"), &known, ONE_HOUR),
        Err(
            "pns: lights quiet: \"3F - Nowhere\" is no lamp, room or zone \
             this can quiet; a mute reaches \"3F - Studio\", \
             \"3F - Studio - HCL3\""
                .to_string()
        ),
        "a place nothing in the config names"
    );
    assert_eq!(
        quiet_command(&typed_at("3f - studio", "30m"), &known, ONE_HOUR),
        Err(
            "pns: lights quiet: \"3f - studio\" is no lamp, room or zone \
             this can quiet; a mute reaches \"3F - Studio\", \
             \"3F - Studio - HCL3\""
                .to_string()
        ),
        "and a case-folded one is a typo rather than a name to forgive, \
         which is how the bridge listing reads it too"
    );
    assert_eq!(
        quiet_command(&typed_at("3F - Studio - HCL3", "30m"), &known, ONE_HOUR),
        Ok(QuietCommand::Mute {
            place: "3F - Studio - HCL3".to_string(),
            seconds: 1_800,
        }),
        "the control: a lamp the config names is stored"
    );
    assert_eq!(
        quiet_command(&typed_at("3F - Nowhere", "off"), &known, ONE_HOUR),
        Ok(QuietCommand::Unmute {
            place: "3F - Nowhere".to_string(),
        }),
        "and `off` is allowed over any name, because it can only remove: a \
         place muted yesterday and dropped from the config today would \
         otherwise be a mute nothing could clear"
    );
    assert_eq!(
        quiet_command(&[], &known, ONE_HOUR),
        Ok(QuietCommand::Report),
        "no argument reports and mutes nothing"
    );
    assert_eq!(
        quiet_command(
            &typed_at("3F - Studio - HCL1", "30m"),
            &places(&[]),
            ONE_HOUR
        ),
        Err(
            "pns: lights quiet: \"3F - Studio - HCL1\" is no lamp, room or zone \
             this can quiet; this config claims no lamp at all, so there is \
             nothing a mute could reach"
                .to_string()
        ),
        "and a config that claims nothing says so rather than trailing off \
         after `a mute reaches`"
    );
    let arguments = vec![
        "3F - Studio".to_string(),
        "30m".to_string(),
        "x".to_string(),
    ];
    assert_eq!(
        quiet_command(&arguments, &known, ONE_HOUR),
        Err(
            "pns: lights quiet takes a place, optionally with a duration \
             or off, or nothing at all"
                .to_string()
        ),
        "arguments: {arguments:?}"
    );
}

/// A schedule an hour away, which is what a bare mute reads.
const ONE_HOUR: Option<u64> = Some(3_600);

#[test]
fn a_bare_mute_lasts_until_the_operators_quiet_hours_end() {
    let known = places(&["3F - Studio"]);
    assert_eq!(
        quiet_command(&[places(&["3F - Studio"])[0].clone()], &known, ONE_HOUR),
        Ok(QuietCommand::Mute {
            place: "3F - Studio".to_string(),
            seconds: 3_600,
        }),
        "no duration typed: the schedule says how long"
    );
    // NO SCHEDULE IS A REFUSAL, never a guessed length: picking one would be
    // a mute the operator did not ask for, ending at an hour they cannot
    // predict.
    assert_eq!(
        quiet_command(&places(&["3F - Studio"]), &known, None),
        Err(
            "pns: lights quiet: a bare mute lasts until your quiet hours end, \
             and `[plugins.hue] quiet_hours` states none; give a duration \
             instead, or set that key"
                .to_string()
        ),
    );
    // AND AN UNKNOWN PLACE IS STILL REFUSED BY NAME on the bare form, which
    // is the same order the two-word form checks in: a typo must not become
    // a mute nothing will ever match.
    assert_eq!(
        quiet_command(&places(&["3F - Nowhere"]), &known, ONE_HOUR),
        Err(unmutable_sentence("3F - Nowhere", &known)),
    );
}

/// The refusal `quiet_command` gives for a place nothing names, so a test
/// asserting it does not restate the sentence.
fn unmutable_sentence(place: &str, known: &[String]) -> String {
    match quiet_command(&places(&[place]), known, Some(1)) {
        Err(said) => said,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

fn places(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_string()).collect()
}

fn typed_at(place: &str, word: &str) -> Vec<String> {
    vec![place.to_string(), word.to_string()]
}
