use super::*;
fn attributes(mode: u32, uid: u32) -> Option<LiveAttributes> {
    Some(LiveAttributes { mode, uid, gid: 20 })
}

#[test]
fn a_root_owned_unwritable_command_directory_is_trusted() {
    assert_eq!(command_trust(true, attributes(0o755, 0)), Ok(()));
    assert_eq!(command_trust(true, attributes(0o700, 0)), Ok(()));
}

#[test]
fn a_relative_privileged_command_is_refused_before_its_attributes() {
    assert_eq!(
        command_trust(false, None),
        Err(CommandTrustRefusal::Relative)
    );
}

#[test]
fn unreadable_command_directory_attributes_are_a_refusal() {
    assert_eq!(
        command_trust(true, None),
        Err(CommandTrustRefusal::Unreadable)
    );
}

#[test]
fn a_non_root_command_directory_is_refused_before_its_mode() {
    assert_eq!(
        command_trust(true, attributes(0o777, 501)),
        Err(CommandTrustRefusal::Owner(501))
    );
}

#[test]
fn group_or_world_writable_command_directories_are_refused() {
    for mode in [0o775, 0o757, 0o777] {
        assert_eq!(
            command_trust(true, attributes(mode, 0)),
            Err(CommandTrustRefusal::Writable(mode))
        );
    }
}

#[test]
fn malformed_restart_bounds_retain_the_thirty_and_five_second_defaults() {
    for text in [
        None,
        Some(""),
        Some("0"),
        Some("01"),
        Some("10000"),
        Some(" 2"),
        Some("-1"),
        Some("1.5"),
    ] {
        let bounds = RestartBounds::parse(text, text);
        assert_eq!(bounds.deadline(), Duration::from_secs(30));
        assert_eq!(bounds.settle(), Duration::from_secs(5));
    }
}

#[test]
fn restart_bounds_accept_one_through_four_decimal_digits_without_leading_zero() {
    for seconds in ["1", "9", "10", "999", "9999"] {
        let bounds = RestartBounds::parse(Some(seconds), Some(seconds));
        let expected = Duration::from_secs(seconds.parse().unwrap());
        assert_eq!(bounds.deadline(), expected);
        assert_eq!(bounds.settle(), expected);
    }
    assert_eq!(RestartBounds::POLL_INTERVAL, Duration::from_millis(250));
}

#[test]
fn parent_identity_accepts_only_the_legacy_nonzero_ten_digit_shape() {
    for (text, expected) in [("1", 1), ("42", 42), ("9999999999", 9_999_999_999)] {
        assert_eq!(ParentPid::parse(text).map(ParentPid::value), Some(expected));
    }
    for text in ["", "0", "01", "-2", "1\r", " 1", "10000000000", "1\n2"] {
        assert_eq!(ParentPid::parse(text), None);
    }
}

#[test]
fn repair_reasons_keep_the_closed_legacy_label_vocabulary() {
    for (verdict, label) in [
        (Drift::Absent, "missing"),
        (Drift::Irregular, "not a regular file"),
        (Drift::Unreadable, "unreadable"),
        (Drift::Content, "content drift"),
        (Drift::Mode, "mode drift"),
        (Drift::Owner, "owner drift"),
        (Drift::Group, "group drift"),
        (Drift::Ok, "drift"),
    ] {
        assert_eq!(verdict.label(), label);
    }
}
