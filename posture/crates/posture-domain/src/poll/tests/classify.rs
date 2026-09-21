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
fn autologin_follows_the_declaration_and_refuses_to_guess_at_an_unreadable_domain() {
    assert_eq!(classify_autologin(Some(true)), Known(On));
    assert_eq!(classify_autologin(Some(false)), Known(Off));
    assert_eq!(classify_autologin(None), Indeterminate);
}
#[test]
fn lulu_base_rules_require_a_readable_preferences_file_with_no_profile_key() {
    assert_eq!(classify_lulu_profile(Some(false)), LuluProfile::Base);
    assert_eq!(classify_lulu_profile(Some(true)), LuluProfile::Active);
    assert_eq!(classify_lulu_profile(None), LuluProfile::Unconfirmed);
}
