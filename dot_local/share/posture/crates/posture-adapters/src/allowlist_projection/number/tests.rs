use super::display_number;

#[test]
fn captured_number_0() {
    assert_eq!(display_number("01"), Some("1".to_owned()));
}

#[test]
fn captured_number_1() {
    assert_eq!(display_number("-01"), Some("-1".to_owned()));
}

#[test]
fn captured_number_2() {
    assert_eq!(display_number("1.00"), Some("1.00".to_owned()));
}

#[test]
fn captured_number_3() {
    assert_eq!(display_number("1e+02"), Some("1E+2".to_owned()));
}

#[test]
fn captured_number_4() {
    assert_eq!(display_number("1E02"), Some("1E+2".to_owned()));
}

#[test]
fn captured_number_5() {
    assert_eq!(display_number("1e400"), Some("1E+400".to_owned()));
}

#[test]
fn captured_number_6() {
    assert_eq!(display_number("-1e400"), Some("-1E+400".to_owned()));
}

#[test]
fn captured_number_7() {
    assert_eq!(display_number("NaN"), Some("null".to_owned()));
}

#[test]
fn captured_number_8() {
    assert_eq!(
        display_number("Infinity"),
        Some("1.7976931348623157e+308".to_owned())
    );
}

#[test]
fn captured_number_9() {
    assert_eq!(
        display_number("9007199254740993"),
        Some("9007199254740993".to_owned())
    );
}

#[test]
fn captured_number_10() {
    assert_eq!(
        display_number("0.00000000000000000000001"),
        Some("1E-23".to_owned())
    );
}

#[test]
fn captured_number_11() {
    assert_eq!(display_number("+1"), Some("1".to_owned()));
}

#[test]
fn captured_number_12() {
    assert_eq!(display_number(".1"), Some("0.1".to_owned()));
}

#[test]
fn captured_number_13() {
    assert_eq!(display_number("-.1"), Some("-0.1".to_owned()));
}

#[test]
fn captured_number_14() {
    assert_eq!(display_number("1."), Some("1".to_owned()));
}

#[test]
fn captured_number_15() {
    assert_eq!(display_number("0001.00"), Some("1.00".to_owned()));
}

#[test]
fn captured_number_16() {
    assert_eq!(display_number("-0"), Some("-0".to_owned()));
}

#[test]
fn captured_number_17() {
    assert_eq!(display_number("-0.0"), Some("-0.0".to_owned()));
}

#[test]
fn captured_number_18() {
    assert_eq!(display_number("0e+2"), Some("0E+2".to_owned()));
}

#[test]
fn captured_number_19() {
    assert_eq!(display_number("0e-7"), Some("0E-7".to_owned()));
}

#[test]
fn captured_number_20() {
    assert_eq!(display_number("0.0000010"), Some("0.0000010".to_owned()));
}

#[test]
fn captured_number_21() {
    assert_eq!(display_number("0.00000010"), Some("1.0E-7".to_owned()));
}

#[test]
fn captured_number_22() {
    assert_eq!(display_number("10e-7"), Some("0.0000010".to_owned()));
}

#[test]
fn captured_number_23() {
    assert_eq!(display_number("10e-8"), Some("1.0E-7".to_owned()));
}

#[test]
fn captured_number_24() {
    assert_eq!(display_number("1e+00002"), Some("1E+2".to_owned()));
}

#[test]
fn captured_number_25() {
    assert_eq!(display_number("nan"), Some("null".to_owned()));
}

#[test]
fn captured_number_26() {
    assert_eq!(display_number("-NaN"), Some("null".to_owned()));
}

#[test]
fn captured_number_27() {
    assert_eq!(display_number("+NaN"), Some("null".to_owned()));
}

#[test]
fn captured_number_28() {
    assert_eq!(display_number("sNaN"), Some("null".to_owned()));
}

#[test]
fn captured_number_29() {
    assert_eq!(display_number("NaN0"), Some("null".to_owned()));
}

#[test]
fn captured_number_30() {
    assert_eq!(display_number("NaN1"), None);
}

#[test]
fn captured_number_31() {
    assert_eq!(
        display_number("inf"),
        Some("1.7976931348623157e+308".to_owned())
    );
}

#[test]
fn captured_number_32() {
    assert_eq!(
        display_number("-Infinity"),
        Some("-1.7976931348623157e+308".to_owned())
    );
}

#[test]
fn captured_number_33() {
    assert_eq!(
        display_number("infinity"),
        Some("1.7976931348623157e+308".to_owned())
    );
}

#[test]
fn captured_number_34() {
    assert_eq!(
        display_number("1e999999999"),
        Some("1E+999999999".to_owned())
    );
}

#[test]
fn captured_number_35() {
    assert_eq!(
        display_number("1e1000000000"),
        Some("1.7976931348623157e+308".to_owned())
    );
}

#[test]
fn captured_number_36() {
    assert_eq!(
        display_number("1e-999999999"),
        Some("1E-999999999".to_owned())
    );
}

#[test]
fn captured_number_37() {
    assert_eq!(
        display_number("1e-1000000000"),
        Some("1E-1000000000".to_owned())
    );
}

#[test]
fn captured_number_38() {
    assert_eq!(
        display_number("0e999999999999999999999"),
        Some("0E+999999999".to_owned())
    );
}

#[test]
fn captured_number_39() {
    assert_eq!(
        display_number("0e-999999999999999999999"),
        Some("0E-1147483646".to_owned())
    );
}

#[test]
fn captured_number_40() {
    assert_eq!(
        display_number("1e-1147483646"),
        Some("1E-1147483646".to_owned())
    );
}

#[test]
fn captured_number_41() {
    assert_eq!(
        display_number("1e-1147483647"),
        Some("0E-1147483646".to_owned())
    );
}

#[test]
fn captured_number_42() {
    assert_eq!(
        display_number("4e-1147483647"),
        Some("0E-1147483646".to_owned())
    );
}

#[test]
fn captured_number_43() {
    assert_eq!(
        display_number("5e-1147483647"),
        Some("1E-1147483646".to_owned())
    );
}

#[test]
fn captured_number_44() {
    assert_eq!(
        display_number("6e-1147483647"),
        Some("1E-1147483646".to_owned())
    );
}

#[test]
fn captured_number_45() {
    assert_eq!(
        display_number("15e-1147483647"),
        Some("2E-1147483646".to_owned())
    );
}

#[test]
fn captured_number_46() {
    assert_eq!(
        display_number("25e-1147483647"),
        Some("3E-1147483646".to_owned())
    );
}

#[test]
fn captured_number_47() {
    assert_eq!(
        display_number("-5e-1147483647"),
        Some("-1E-1147483646".to_owned())
    );
}

#[test]
fn captured_number_48() {
    assert_eq!(
        display_number("99e999999999"),
        Some("1.7976931348623157e+308".to_owned())
    );
}

#[test]
fn captured_number_49() {
    assert_eq!(
        display_number("9.9e999999999"),
        Some("9.9E+999999999".to_owned())
    );
}

#[test]
fn captured_number_50() {
    assert_eq!(
        display_number("0e1000000000"),
        Some("0E+999999999".to_owned())
    );
}

#[test]
fn captured_number_51() {
    assert_eq!(display_number("NaN000"), Some("null".to_owned()));
}

#[test]
fn captured_number_52() {
    assert_eq!(display_number("sNaN0"), Some("null".to_owned()));
}

#[test]
fn captured_number_53() {
    assert_eq!(
        display_number("+Inf"),
        Some("1.7976931348623157e+308".to_owned())
    );
}

#[test]
fn captured_number_54() {
    assert_eq!(
        display_number("INF"),
        Some("1.7976931348623157e+308".to_owned())
    );
}
