use super::*;

fn pin() -> Pin {
    Pin::new(b"pin.example.test", b"10.0.0.5", b"pin").expect("a usable pin")
}

/// The record is the three columns, tab separated, and nothing else.
#[test]
fn the_record_is_the_three_columns_separated_by_tabs() {
    assert_eq!(pin().record(), b"10.0.0.5\tpin.example.test\tpin");
}

/// A pin owns BOTH its names, because the fallback exists to answer for both.
#[test]
fn a_pin_owns_its_short_name_as_well_as_its_long_one() {
    assert_eq!(pin().names(), [b"pin.example.test".as_slice(), b"pin"]);
}

/// A field that is not one column would write extra columns, or extra LINES,
/// into a file being rewritten as root. Each is refused BY NAME.
#[test]
fn a_field_that_is_not_one_column_is_refused_by_name() {
    for byte in [b' ', b'\t', b'\n', b'\r', 0x0c, b'#'] {
        let smuggled = [b"pin", [byte].as_slice(), b"more"].concat();
        assert_eq!(
            Pin::new(&smuggled, b"10.0.0.5", b"pin"),
            Err(Field::Fqdn),
            "byte {byte:#04x} passed as a column"
        );
        assert_eq!(
            Pin::new(b"pin.example.test", &smuggled, b"pin"),
            Err(Field::Ip)
        );
        assert_eq!(
            Pin::new(b"pin.example.test", b"10.0.0.5", &smuggled),
            Err(Field::Short)
        );
    }
}

/// An empty field is not a column either: it would collapse two separators into
/// one and shift every column left.
#[test]
fn an_empty_field_is_refused() {
    assert_eq!(Pin::new(b"", b"10.0.0.5", b"pin"), Err(Field::Fqdn));
    assert_eq!(Pin::new(b"pin.example.test", b"", b"pin"), Err(Field::Ip));
    assert_eq!(
        Pin::new(b"pin.example.test", b"10.0.0.5", b""),
        Err(Field::Short)
    );
}

/// Other Unicode spaces split no hosts record, so none of them can smuggle a
/// column and none of them is refused.
#[test]
fn a_space_that_splits_no_record_is_not_refused() {
    // A no-break space and an em space, as their UTF-8 bytes.
    assert!(Pin::new("pin\u{00a0}one.test".as_bytes(), b"10.0.0.5", b"pin").is_ok());
    assert!(Pin::new("pin\u{2003}one.test".as_bytes(), b"10.0.0.5", b"pin").is_ok());
}

/// All three conditions, and the file is done.
#[test]
fn a_file_with_one_correct_terminated_record_is_converged() {
    assert!(
        Survey {
            claiming_lines: 1,
            desired_record_present: true,
            ends_with_terminator: true,
        }
        .is_converged()
    );
}

/// EXACTLY one, not at least one. A correct line plus a stale duplicate leaves
/// two lines naming the pin and the resolver picking one.
#[test]
fn a_correct_record_beside_a_stale_duplicate_is_not_converged() {
    assert!(
        !Survey {
            claiming_lines: 2,
            desired_record_present: true,
            ends_with_terminator: true,
        }
        .is_converged()
    );
}

/// A line that claims the pin's name but is not its record is a stale pin,
/// which is the case this whole tool exists for.
#[test]
fn a_claiming_line_that_is_not_the_record_is_not_converged() {
    assert!(
        !Survey {
            claiming_lines: 1,
            desired_record_present: false,
            ends_with_terminator: true,
        }
        .is_converged()
    );
}

/// The terminator is the condition only a rebuild can satisfy: the resolver
/// reads an unterminated final line one byte short of what the file holds.
#[test]
fn a_correct_record_on_an_unterminated_final_line_is_not_converged() {
    assert!(
        !Survey {
            claiming_lines: 1,
            desired_record_present: true,
            ends_with_terminator: false,
        }
        .is_converged()
    );
}

/// A file with no pin at all is not converged; it is the ordinary first run.
#[test]
fn a_file_that_never_heard_of_the_pin_is_not_converged() {
    assert!(!Survey::default().is_converged());
}
