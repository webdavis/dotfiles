use super::*;

fn strings(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| (*word).to_string()).collect()
}

#[test]
fn the_color_flag_answers_before_the_subcommand_and_after_it() {
    for argv in [
        strings(&["--no-color", "doctor"]),
        strings(&["doctor", "--no-color"]),
    ] {
        let (flagless, forced_plain) = take_tool_wide_flags(&argv);
        assert!(forced_plain, "{argv:?}");
        assert_eq!(flagless, strings(&["doctor"]), "{argv:?}");
    }
}

#[test]
fn argv_without_the_flag_is_passed_through_unchanged() {
    let argv = strings(&["gateway", "schedule", "--id", "x"]);
    let (flagless, forced_plain) = take_tool_wide_flags(&argv);
    assert!(!forced_plain);
    assert_eq!(flagless, argv);
}

#[test]
fn the_flag_typed_twice_is_still_one_answer_and_leaves_nothing_behind() {
    let argv = strings(&["--no-color", "lights", "--no-color", "tick"]);
    let (flagless, forced_plain) = take_tool_wide_flags(&argv);
    assert!(forced_plain);
    assert_eq!(flagless, strings(&["lights", "tick"]));
}

#[test]
fn a_verb_keeps_its_position_when_the_flag_was_typed_before_the_subcommand() {
    // The bug this exists to prevent: reading the verb off the environment
    // handed `gateway` to the gateway as its own verb.
    let (flagless, _) = take_tool_wide_flags(&strings(&["--no-color", "gateway", "start"]));
    assert_eq!(second_argument(&flagless), "start");
}

#[test]
fn a_word_that_merely_contains_the_flag_is_not_the_flag() {
    let argv = strings(&["recap", "--since-epoch=--no-color"]);
    let (flagless, forced_plain) = take_tool_wide_flags(&argv);
    assert!(!forced_plain);
    assert_eq!(flagless, argv);
}

#[test]
fn both_input_forms_are_read_off_the_one_send_subcommand() {
    assert_eq!(
        SendForm::of(&strings(&["--producer", "lights", "--state", "done"])),
        SendForm::Flags
    );
    assert_eq!(SendForm::of(&strings(&["--json"])), SendForm::Envelope);
}

#[test]
fn json_in_a_value_position_is_still_just_a_value() {
    // `--detail`'s own value could legitimately spell `--json`; only the
    // leading token names the form.
    assert_eq!(
        SendForm::of(&strings(&[
            "--producer",
            "x",
            "--state",
            "done",
            "--detail",
            "--json"
        ])),
        SendForm::Flags
    );
}

#[test]
fn the_color_flag_answers_in_either_position_for_the_envelope_form_too() {
    for args in [
        strings(&["--no-color", "--json"]),
        strings(&["--json", "--no-color"]),
    ] {
        let (filtered, _) = take_tool_wide_flags(&args);
        assert_eq!(SendForm::of(&filtered), SendForm::Envelope, "{args:?}");
        assert_eq!(filtered, strings(&["--json"]), "{args:?}");
    }
}

#[test]
fn the_bare_event_path_is_refused_rather_than_delivered_empty() {
    // The whole point of the subcommand: argv that used to render an empty
    // event and notify about it now earns the usage text and exit 2.
    for first in ["--state", "--producer", "stpo", ""] {
        let usage = Usage::of(first);
        assert_eq!(usage, Usage::Refused, "{first:?}");
        assert_eq!(usage.exit_code(), 2, "{first:?}");
    }
}

#[test]
fn help_with_no_subcommand_still_prints_and_exits_zero() {
    for first in ["--help", "-h"] {
        let usage = Usage::of(first);
        assert_eq!(usage, Usage::Requested, "{first}");
        assert!(!usage.refused(), "{first}");
        assert_eq!(usage.exit_code(), 0, "{first}");
    }
}

#[test]
fn send_hands_on_what_followed_it_and_keeps_a_value_spelling_the_color_flag() {
    for argv in [
        strings(&["send", "--detail", "--no-color"]),
        strings(&["--no-color", "send", "--detail", "--no-color"]),
    ] {
        assert_eq!(
            arguments_after_send(&argv),
            strings(&["--detail", "--no-color"]),
            "{argv:?}"
        );
    }
}
