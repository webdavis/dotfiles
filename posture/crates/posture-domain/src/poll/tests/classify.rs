use super::*;
use ControlReading::*;
use ControlValue::*;
#[test]
fn failed_or_conflicting_probes_never_believe_healthy_printed_text() {
    let needles = [("yes", On), ("also yes", On), ("no", Off)];
    assert_eq!(classify_messages("yes also yes", 0, &needles), Known(On));
    for exit in [-1, 1, 2, 124] {
        assert_eq!(classify_messages("yes", exit, &needles), Indeterminate);
    }
    for text in ["", "unreadable", "yes no", "yes no yes"] {
        assert_eq!(classify_messages(text, 0, &needles), Indeterminate);
    }
}
#[test]
fn pid_status_and_output_must_agree_in_both_directions() {
    for output in ["0", "42", "1\n2", "99999999999999999999999"] {
        assert_eq!(classify_pgrep(output, 0), Known(Running));
    }
    assert_eq!(classify_pgrep("", 1), Known(Stopped));
    for (text, exit) in [
        ("", 0),
        ("1", 1),
        ("1", 2),
        ("", 2),
        ("1\n", 0),
        ("1 2", 0),
        ("-1", 0),
        ("é", 0),
    ] {
        assert_eq!(classify_pgrep(text, exit), Indeterminate, "{text:?}/{exit}");
    }
}
#[test]
fn all_five_filevault_forms_preserve_deferred_enablement_as_off() {
    for (text, value) in [
        ("FileVault is On.", On),
        ("FileVault is On, but needs to be restarted to finish.", On),
        ("FileVault is Off.", Off),
        (
            "FileVault is Off, but will be enabled after the next restart.",
            Off,
        ),
        (
            "FileVault is Off, but needs to be restarted to finish.",
            Off,
        ),
    ] {
        assert_eq!(classify_filevault(text, 0), Known(value));
        assert_eq!(classify_filevault(text, 1), Indeterminate);
    }
    assert_eq!(
        classify_filevault("FileVault is On. FileVault is Off.", 0),
        Indeterminate
    );
}
#[test]
fn autologin_checks_declaration_presence_and_only_the_exact_absence_diagnostic() {
    for value in ["", "stephen", "false", "autoLoginUser) does not exist"] {
        assert_eq!(classify_autologin(value, 0), Known(On));
    }
    assert_eq!(
        classify_autologin("prefix autoLoginUser) does not exist suffix", 1),
        Known(Off)
    );
    for text in [
        "",
        "does not exist",
        "autoLoginUser does not exist",
        "access denied",
    ] {
        assert_eq!(classify_autologin(text, 1), Indeterminate);
    }
}
#[test]
fn lulu_base_rules_require_a_successful_nonempty_read_with_no_profile_key() {
    assert_eq!(classify_lulu_profile(true, true, false), LuluProfile::Base);
    assert_eq!(classify_lulu_profile(true, true, true), LuluProfile::Active);
    for (success, nonempty) in [(false, true), (true, false), (false, false)] {
        assert_eq!(
            classify_lulu_profile(success, nonempty, false),
            LuluProfile::Unconfirmed
        );
    }
}
