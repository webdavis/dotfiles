use super::*;
use crate::{InspectionFailure, SshCommandResult, SshCompleted, SshScanFailure};
use posture_domain::{SSH_LOCAL_ADDRESS_SAMPLES, SshRecord, SshTreeRefusal};
use std::{cell::Cell, collections::VecDeque};

const HARDENED: &str = "passwordauthentication no\nkbdinteractiveauthentication no\nusepam yes\npubkeyauthentication yes\npermitrootlogin no\ngssapiauthentication no\nhostbasedauthentication no\n";
struct Ssh {
    available: bool,
    answers: VecDeque<SshCommandResult>,
    calls: Vec<String>,
}
impl Sshd for Ssh {
    fn available(&self) -> bool {
        self.available
    }
    fn global(&mut self) -> SshCommandResult {
        self.calls.push("global".into());
        self.answers.pop_front().unwrap()
    }
    fn connection(&mut self, spec: &str) -> SshCommandResult {
        self.calls.push(spec.into());
        self.answers.pop_front().unwrap()
    }
    fn syntax(&mut self) -> SshCommandResult {
        panic!("verify never checks privileged syntax")
    }
}
struct Tree {
    reads: Cell<usize>,
    fail: bool,
}
impl SshTree for Tree {
    fn scan(&self) -> Vec<SshScanFailure> {
        self.reads.set(self.reads.get() + 1);
        if self.fail {
            vec![SshScanFailure::File(SshTreeRefusal::Cycle(b"bad".to_vec()))]
        } else {
            vec![]
        }
    }
    fn observe(&self) -> Result<Vec<SshRecord>, SshScanFailure> {
        panic!("verify uses the Match scan")
    }
}
fn healthy() -> SshCommandResult {
    Ok(SshCompleted {
        status: 0,
        output: HARDENED.as_bytes().to_vec(),
    })
}
fn refusal(verdict: &str) -> SshCommandResult {
    Ok(SshCompleted {
        status: 0,
        output: format!("refuseconnection {verdict}\n").into_bytes(),
    })
}
/// The global resolve and the two user samples answer hardened; each local
/// address sample answers the verdict policy demands of it, read off the shared
/// sample list so a sample added to policy is exercised here without an edit.
fn fixture() -> (Ssh, Tree) {
    let mut answers = VecDeque::from([healthy(), healthy(), healthy()]);
    answers.extend(SSH_LOCAL_ADDRESS_SAMPLES.map(|(_, verdict)| refusal(verdict)));
    (
        Ssh {
            available: true,
            answers,
            calls: vec![],
        },
        Tree {
            reads: Cell::new(0),
            fail: false,
        },
    )
}
/// Index of the answer serving one local address sample, past the global
/// resolve and the two user samples.
fn sample(address: &str) -> usize {
    3 + SSH_LOCAL_ADDRESS_SAMPLES
        .iter()
        .position(|(candidate, _)| *candidate == address)
        .expect("the sample list names this address")
}
fn context() -> SshVerifyContext<'static> {
    SshVerifyContext {
        user: &|| Some("operator".into()),
        executable: Path::new("/fixture/sshd"),
        allow_missing: false,
    }
}

#[test]
fn verify_requires_global_match_scan_and_both_exact_connection_samples() {
    let (mut ssh, tree) = fixture();
    assert_eq!(
        verify_ssh(&mut ssh, &tree, &context()),
        SshVerification::Verified
    );
    assert_eq!(tree.reads.get(), 1);
    assert_eq!(
        ssh.calls,
        [
            "global",
            "user=root,host=localhost,addr=127.0.0.1",
            "user=operator,host=localhost,addr=127.0.0.1",
            "user=operator,host=localhost,addr=203.0.113.1,laddr=100.64.0.1",
            "user=operator,host=localhost,addr=203.0.113.1,laddr=fd7a:115c:a1e0::1",
            "user=operator,host=localhost,addr=203.0.113.1,laddr=127.0.0.1",
            "user=operator,host=localhost,addr=203.0.113.1,laddr=::1",
            "user=operator,host=localhost,addr=203.0.113.1,laddr=192.168.0.1",
            "user=operator,host=localhost,addr=203.0.113.1,laddr=fd00::1"
        ]
    );
}

/// Without the Match block every local address resolves the sshd default, so
/// both refused samples report the wrong verdict and the allowed ones still
/// pass. A verify that reported this tree hardened would be the whole failure.
#[test]
fn a_tree_missing_the_refusal_block_fails_on_exactly_the_addresses_it_should_refuse() {
    let (mut ssh, tree) = fixture();
    for (address, _) in SSH_LOCAL_ADDRESS_SAMPLES {
        ssh.answers[sample(address)] = refusal("no");
    }
    let SshVerification::Failed(failures) = verify_ssh(&mut ssh, &tree, &context()) else {
        panic!("fail closed")
    };
    assert_eq!(failures.len(), 2, "{failures:?}");
    assert!(failures[0].contains("laddr=192.168.0.1"));
    assert!(failures[1].contains("laddr=fd00::1"));
    assert!(
        failures
            .iter()
            .all(|failure| failure.contains("'refuseconnection' is 'no', want 'yes'"))
    );
}

/// A later file or a Match block that resolves FIRST can re-admit one address
/// while the rest of the policy still holds, which is the shape a bare
/// `sshd -G` cannot see at all. The sample that names the re-admitted address
/// is what catches it.
#[test]
fn a_re_allowed_local_address_fails_even_while_every_other_sample_still_holds() {
    let (mut ssh, tree) = fixture();
    ssh.answers[sample("192.168.0.1")] = refusal("no");
    let SshVerification::Failed(failures) = verify_ssh(&mut ssh, &tree, &context()) else {
        panic!("fail closed")
    };
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(failures[0].contains("local address check"));
    assert!(failures[0].contains("laddr=192.168.0.1"));
}

/// An unreadable resolve is a failed verify, never a pass by silence: the
/// refusal verdict is absent rather than wrong, and it is still reported.
#[test]
fn a_local_address_resolve_that_says_nothing_about_refusal_fails_closed() {
    for answer in [
        Ok(SshCompleted {
            status: 0,
            output: b"usepam yes\n".to_vec(),
        }),
        Err(InspectionFailure::TimedOut),
    ] {
        let (mut ssh, tree) = fixture();
        ssh.answers[sample("192.168.0.1")] = answer;
        let SshVerification::Failed(failures) = verify_ssh(&mut ssh, &tree, &context()) else {
            panic!("fail closed")
        };
        assert_eq!(failures.len(), 1, "{failures:?}");
        assert!(failures[0].contains("local address check"));
    }
}
#[test]
fn failed_global_and_tree_reads_do_not_skip_connection_checks() {
    let (mut ssh, mut tree) = fixture();
    tree.fail = true;
    ssh.answers[0] = Err(InspectionFailure::TimedOut);
    ssh.answers[1] = Ok(SshCompleted {
        status: 7,
        output: b"fixture error".to_vec(),
    });
    let SshVerification::Failed(failures) = verify_ssh(&mut ssh, &tree, &context()) else {
        panic!("fail closed")
    };
    assert_eq!(failures.len(), 3, "{failures:?}");
    assert!(failures[0].contains("124"));
    assert!(failures[1].contains("cycle"));
    assert!(failures[2].contains("fixture error"));
    assert_eq!(ssh.calls.len(), 3 + SSH_LOCAL_ADDRESS_SAMPLES.len());
}
#[test]
fn missing_binary_only_skips_when_explicitly_allowed_and_does_no_reads() {
    for allow_missing in [false, true] {
        let (mut ssh, tree) = fixture();
        ssh.available = false;
        let c = SshVerifyContext {
            allow_missing,
            ..context()
        };
        let outcome = verify_ssh(&mut ssh, &tree, &c);
        if allow_missing {
            assert_eq!(outcome, SshVerification::Skipped);
        } else {
            let SshVerification::Failed(failures) = outcome else {
                panic!("fail closed")
            };
            assert_eq!(failures.len(), 1);
        }
        assert!(ssh.calls.is_empty());
        assert_eq!(tree.reads.get(), 0);
    }
}
#[test]
fn missing_identity_fails_after_global_and_scan_without_guessing_root() {
    for user in [None, Some("")] {
        let (mut ssh, tree) = fixture();
        let lookup = || user.map(str::to_owned);
        let c = SshVerifyContext {
            user: &lookup,
            ..context()
        };
        let SshVerification::Failed(failures) = verify_ssh(&mut ssh, &tree, &c) else {
            panic!("fail closed")
        };
        assert_eq!(failures.len(), 1);
        assert_eq!(ssh.calls, ["global"]);
        assert_eq!(tree.reads.get(), 1);
    }
}
#[test]
fn wrong_and_absent_directives_are_reported_separately_with_each_check_label() {
    let (mut ssh, tree) = fixture();
    ssh.answers[1] = Ok(SshCompleted {
        status: 0,
        output: b"passwordauthentication yes\n".to_vec(),
    });
    let SshVerification::Failed(failures) = verify_ssh(&mut ssh, &tree, &context()) else {
        panic!("fail closed")
    };
    assert_eq!(failures.len(), 7);
    assert!(failures[0].contains("'yes', want 'no'"));
    assert!(failures[1].contains("is absent"));
    assert!(failures.iter().all(|f| f.contains("user=root")));
}
