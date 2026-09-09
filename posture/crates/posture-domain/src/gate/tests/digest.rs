use super::*;
use Detector::*;

#[test]
fn c3c_authentication_configuration_changes_only_digest() {
    for action in [Action::Added, Action::Removed, Action::Other] {
        let row = GateFinding {
            action,
            ..finding(AgentAuthfileChanged)
        };
        assert_eq!(route(row), digest());
        assert_eq!(
            gate(
                row,
                GateEvidence {
                    severity: None,
                    ..evidence(row)
                },
                |_| false
            ),
            digest()
        );
    }
}

#[test]
fn added_listeners_digest() {
    assert_eq!(route(finding(ListeningPortsNonLoopback)), digest());
}

#[test]
fn private_keys_sudoers_and_other_ssh_files_digest() {
    for target in [
        "/x/id_rsa",
        "/x/config",
        "/x/authorized_keys.old",
        "/x/authorized_keys/",
        "",
    ] {
        assert_eq!(route(file(FileCategory::Ssh, target)), digest());
    }
    assert_eq!(route(file(FileCategory::Sudoers, "/etc/sudoers")), digest());
}

#[test]
fn system_extensions_digest_without_untrusted_promotion() {
    assert_eq!(route(finding(SystemExtensionsNew)), digest());
    assert_eq!(
        signed(finding(SystemExtensionsNew), "signed: Apple", false),
        GateOutcome::Digest {
            signing: Some("signed: Apple")
        }
    );
}

#[test]
fn failed_enricher_stdout_digests_without_promotion() {
    assert_eq!(
        signed(finding(SystemExtensionsNew), "partial signing fact", false),
        GateOutcome::Digest {
            signing: Some("partial signing fact")
        }
    );
}
