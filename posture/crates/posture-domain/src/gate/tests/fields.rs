use super::*;

const GOOD: LaunchdIdentity<'static> = LaunchdIdentity {
    label: "com.good",
    path: "/fixture/Library/LaunchAgents/com.good.plist",
    program: "/fixture/bin/good",
};

fn route_identity(identity: LaunchdIdentity<'_>) -> GateOutcome<'_> {
    let mut row = finding(Detector::PersistenceLaunchd);
    row.columns.launchd = identity;
    gate(row, evidence(row), |received| {
        assert_eq!(received, identity);
        received == GOOD
    })
}

#[test]
fn hostile_unit_separator_in_path_cannot_impersonate_the_allowlisted_tuple() {
    let path = format!("{}\u{1f}{}\u{1f}{}", GOOD.path, GOOD.label, GOOD.program);
    assert_eq!(
        route_identity(LaunchdIdentity {
            label: "com.attacker",
            path: &path,
            program: "/attacker/mal"
        }),
        page()
    );
}

#[test]
fn hostile_newline_in_label_stays_one_field() {
    assert_eq!(
        route_identity(LaunchdIdentity {
            label: "com.attacker\ncom.good",
            ..GOOD
        }),
        page()
    );
}

#[test]
fn hostile_control_genuine_tuple_is_suppressed() {
    assert_eq!(route_identity(GOOD), GateOutcome::LogOnly);
}

#[test]
fn hostile_tab_in_program_stays_one_field() {
    assert_eq!(
        route_identity(LaunchdIdentity {
            program: "/fixture/bin/evil\tcom.good",
            ..GOOD
        }),
        page()
    );
}

#[test]
fn an_empty_field_does_not_shift_the_other_fields() {
    assert_eq!(
        route_identity(LaunchdIdentity { label: "", ..GOOD }),
        page()
    );
    assert_eq!(route_identity(LaunchdIdentity { path: "", ..GOOD }), page());
    assert_eq!(
        route_identity(LaunchdIdentity {
            program: "",
            ..GOOD
        }),
        page()
    );
}
