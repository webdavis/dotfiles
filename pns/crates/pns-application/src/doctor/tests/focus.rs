use crate::{FocusReading, doctor_focus};
use std::io::ErrorKind;

#[test]
fn doctor_focus_distinguishes_off_absent_and_unreadable_without_reading_when_off() {
    assert!(
        doctor_focus(true, false, || panic!("no modes named must not read"))
            .contains("awareness is off")
    );
    assert!(
        doctor_focus(true, true, || Err(ErrorKind::NotFound))
            .contains("no Focus database was found")
    );
    assert_eq!(
        doctor_focus(true, true, || Err(ErrorKind::PermissionDenied)),
        "pns doctor: the Focus database could not be read, so Focus is being ignored (permission denied)."
    );
}

#[test]
fn doctor_focus_says_the_switch_is_off_even_while_a_roster_still_names_modes() {
    let line = doctor_focus(false, true, || panic!("switched off must not read"));
    assert!(line.contains("[focus] enabled = false"));
    assert!(line.contains("modes still listed"));
}

#[test]
fn doctor_focus_reports_catalog_failure_beside_the_same_silence_reading() {
    assert_eq!(
        doctor_focus(true, true, || Ok(FocusReading {
            silenced: false,
            catalog: None
        })),
        "pns doctor: no macOS Focus you named is active"
    );
    let line = doctor_focus(true, true, || {
        Ok(FocusReading {
            silenced: true,
            catalog: Some(ErrorKind::PermissionDenied),
        })
    });
    assert!(line.starts_with("pns doctor: a macOS Focus you named is ON"));
    assert!(line.contains("mode catalog could not be read (permission denied)"));
    assert!(line.ends_with("only a raw modeIdentifier still would"));
}
