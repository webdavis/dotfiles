use super::*;

fn record() -> ControlRecord<'static> {
    ControlRecord {
        id: "filevault",
        tier: "verify",
        reader: "fdesetup_status",
        expect: "on",
        target: "",
        description: "FileVault",
        remedy: "",
    }
}
fn accepted(records: &[ControlRecord<'_>]) -> Vec<Control> {
    validate_controls(ControlsInput::Records(records)).unwrap()
}
fn refused(records: &[ControlRecord<'_>], kind: ControlsRefusalKind) {
    assert_eq!(
        validate_controls(ControlsInput::Records(records))
            .unwrap_err()
            .kind,
        kind
    );
}

#[test]
fn a_controls_document_must_be_present_and_exactly_one_array() {
    assert_eq!(accepted(&[record()]).len(), 1);
    assert_eq!(
        validate_controls(ControlsInput::Missing("/missing"))
            .unwrap_err()
            .kind,
        ControlsRefusalKind::Missing
    );
    assert_eq!(
        validate_controls(ControlsInput::Malformed)
            .unwrap_err()
            .kind,
        ControlsRefusalKind::Malformed
    );
}
#[test]
fn a_control_count_accepts_one_and_two_but_refuses_zero() {
    assert_eq!(accepted(&[record()]).len(), 1);
    assert_eq!(
        accepted(&[
            record(),
            ControlRecord {
                id: "second",
                ..record()
            }
        ])
        .len(),
        2
    );
    refused(&[], ControlsRefusalKind::Empty);
}
#[test]
fn control_ids_accept_exact_ascii_ranges_and_refuse_their_neighbors() {
    for id in ["a", "z", "0", "9", "_", "a0_z9"] {
        assert_eq!(accepted(&[ControlRecord { id, ..record() }])[0].id, id);
    }
    for id in ["", "`", "{", "/", ":", "^", "A", "a-b", "é"] {
        refused(&[ControlRecord { id, ..record() }], ControlsRefusalKind::Id);
    }
}
#[test]
fn each_reserved_or_duplicate_id_refuses_the_whole_control_set() {
    assert_eq!(
        accepted(&[
            record(),
            ControlRecord {
                id: "firewall_",
                ..record()
            }
        ])
        .len(),
        2
    );
    for id in ["firewall", "gatekeeper", "screenlock", "filevault"] {
        refused(
            &[record(), ControlRecord { id, ..record() }],
            ControlsRefusalKind::Collision,
        );
    }
}
#[test]
fn only_the_exact_verify_tier_is_admitted() {
    assert_eq!(accepted(&[record()]).len(), 1);
    for tier in ["verifx", "verifz", "Verify", "verify ", ""] {
        refused(
            &[ControlRecord { tier, ..record() }],
            ControlsRefusalKind::Tier,
        );
    }
}
#[test]
fn all_eight_readers_keep_their_exact_domains_and_target_requirements() {
    for (reader, values, target) in [
        ("fdesetup_status", ["on", "off"], ""),
        ("defaults_autologin", ["on", "off"], ""),
        ("csrutil_status", ["enabled", "disabled"], ""),
        ("sysadminctl_guest", ["enabled", "disabled"], ""),
        ("pgrep_oversight", ["running", "stopped"], ""),
        ("pgrep_lulu_extension", ["running", "stopped"], ""),
        ("lulu_rule_present", ["present", "absent"], "/"),
        ("lulu_rule_resolved_present", ["present", "absent"], "/"),
    ] {
        for expect in values {
            assert_eq!(
                accepted(&[ControlRecord {
                    reader,
                    expect,
                    target,
                    ..record()
                }])
                .len(),
                1
            );
        }
        for unknown in [format!("{reader}x"), format!(" {reader}")] {
            refused(
                &[ControlRecord {
                    reader: &unknown,
                    expect: values[0],
                    target,
                    ..record()
                }],
                ControlsRefusalKind::Reader,
            );
        }
    }
}
#[test]
fn a_reader_expectation_cannot_cross_into_another_domain_or_an_empty_value() {
    assert_eq!(accepted(&[record()])[0].expect, ControlValue::On);
    assert_eq!(
        accepted(&[ControlRecord {
            expect: "off",
            ..record()
        }])[0]
            .expect,
        ControlValue::Off
    );
    for expect in ["", "on ", "onn", "enabled", "indeterminate"] {
        refused(
            &[ControlRecord { expect, ..record() }],
            ControlsRefusalKind::Expect,
        );
    }
}
#[test]
fn targets_are_required_only_for_the_two_rule_readers_in_both_directions() {
    assert_eq!(accepted(&[record()]).len(), 1);
    refused(
        &[ControlRecord {
            target: "/",
            ..record()
        }],
        ControlsRefusalKind::UnexpectedTarget,
    );
    for reader in ["lulu_rule_present", "lulu_rule_resolved_present"] {
        assert_eq!(
            accepted(&[ControlRecord {
                reader,
                expect: "present",
                target: "/",
                ..record()
            }])
            .len(),
            1
        );
        refused(
            &[ControlRecord {
                reader,
                expect: "present",
                ..record()
            }],
            ControlsRefusalKind::MissingTarget,
        );
    }
}
#[test]
fn a_target_requires_the_absolute_prefix_and_refuses_only_the_captured_line_separators() {
    let rule = ControlRecord {
        reader: "lulu_rule_present",
        expect: "present",
        ..record()
    };
    for target in ["/", "/x", "/x\ry", "/x\ty", "/x\u{1e}y", "/x y"] {
        assert_eq!(
            accepted(&[ControlRecord { target, ..rule }])[0].target,
            target
        );
    }
    for target in [".", "0", "x", "~/x"] {
        refused(
            &[ControlRecord { target, ..rule }],
            ControlsRefusalKind::RelativeTarget,
        );
    }
    for target in ["/x\ny", "/x\u{1f}y"] {
        refused(
            &[ControlRecord { target, ..rule }],
            ControlsRefusalKind::MultilineTarget,
        );
    }
}
#[test]
fn description_admission_uses_its_sanitized_value_and_caps_both_fields_at_160() {
    assert_eq!(
        accepted(&[ControlRecord {
            description: " ",
            ..record()
        }])[0]
            .description,
        " "
    );
    for description in ["", "`", "\"$'\\"] {
        refused(
            &[ControlRecord {
                description,
                ..record()
            }],
            ControlsRefusalKind::Description,
        );
    }
    for size in [159, 160, 161] {
        let text = "é".repeat(size);
        let admitted = accepted(&[ControlRecord {
            description: &text,
            remedy: &text,
            ..record()
        }]);
        assert_eq!(admitted[0].description, "é".repeat(size.min(160)));
        assert_eq!(admitted[0].remedy, admitted[0].description);
    }
    assert_eq!(control_span("[x]\n\r\t`$'\"\\"), "`[x]   `");
}
#[test]
fn an_invalid_later_control_never_returns_a_partially_monitored_prefix() {
    assert_eq!(accepted(&[record()]).len(), 1);
    assert!(
        validate_controls(ControlsInput::Records(&[
            record(),
            ControlRecord {
                id: "bad-id",
                ..record()
            }
        ]))
        .is_err()
    );
}
