use super::*;
use crate::*;

fn route(identity: LaunchdIdentity<'_>, untrusted: bool, vouch: bool) -> GateOutcome<'_> {
    gate(
        GateFinding {
            detector: Detector::PersistenceLaunchd,
            action: Action::Added,
            enrichment_path: identity.path,
            columns: GateColumns {
                launchd: identity,
                ..GateColumns::default()
            },
        },
        GateEvidence {
            severity: Some(Severity::Notice),
            signing: Some(Signing {
                untrusted,
                text: "fixture signing",
            }),
            integrity: IntegrityVerdict::Page,
            triage: None,
        },
        |identity| {
            judge(Allowlist::Read(&[entry()]), identity, None, |_| vouch)
                == AllowlistVerdict::Suppress
        },
    )
}

#[test]
fn c4a_full_allowlisted_tuple_is_suppressed() {
    assert_eq!(route(ID, false, true), GateOutcome::LogOnly);
}

#[test]
fn c4b_reused_label_pages() {
    assert!(matches!(
        route(
            LaunchdIdentity {
                program: "/other",
                ..ID
            },
            false,
            true
        ),
        GateOutcome::Page { .. }
    ));
}

#[test]
fn c4c_unknown_user_agent_pages() {
    assert!(matches!(
        route(
            LaunchdIdentity {
                label: "org.other",
                ..ID
            },
            false,
            true
        ),
        GateOutcome::Page { .. }
    ));
}

#[test]
fn c4d_allowlisted_but_untrusted_program_pages() {
    assert!(matches!(route(ID, true, true), GateOutcome::Page { .. }));
}

#[test]
fn with_nothing_able_to_vouch_an_own_agent_pages() {
    assert!(matches!(route(ID, false, false), GateOutcome::Page { .. }));
}
