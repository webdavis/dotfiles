use super::*;
use crate::{InspectionFailure, SshCommandResult, SshCompleted, SshFile};
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
};

fn completed(text: &str) -> SshCommandResult {
    Ok(SshCompleted {
        status: 0,
        output: text.as_bytes().to_vec(),
    })
}
struct Files {
    exists: bool,
    refuse: bool,
    pretend: bool,
    calls: Vec<String>,
}
impl SshInstallFiles for Files {
    fn path(&self, _: SshFile) -> PathBuf {
        PathBuf::from("/fixture/000-ssh-hardening.conf")
    }
    fn directory_exists(&self) -> bool {
        panic!("rollback doesn't require the directory")
    }
    fn exists(&self, file: SshFile) -> Result<bool, InspectionFailure> {
        assert_eq!(file, SshFile::Target);
        Ok(self.exists)
    }
    fn prime(&mut self) -> SshCommandResult {
        self.calls.push("prime".into());
        completed("")
    }
    fn remove(&mut self, files: &[SshFile]) -> SshCommandResult {
        assert_eq!(files, &[SshFile::Target]);
        self.calls.push("remove".into());
        if self.refuse {
            Err(InspectionFailure::Failed)
        } else {
            if !self.pretend {
                self.exists = false;
            }
            completed("")
        }
    }
    fn stage(&mut self, _: &[u8]) -> SshCommandResult {
        panic!("no stage")
    }
    fn chmod(&mut self) -> SshCommandResult {
        panic!("no chmod")
    }
    fn save(&mut self) -> SshCommandResult {
        panic!("no save")
    }
    fn rename(&mut self, _: SshFile, _: SshFile) -> SshCommandResult {
        panic!("no rename")
    }
}
struct Ssh {
    available: bool,
    answers: VecDeque<SshCommandResult>,
    specs: Vec<String>,
}
impl Sshd for Ssh {
    fn available(&self) -> bool {
        self.available
    }
    fn global(&mut self) -> SshCommandResult {
        panic!("password proof needs connection resolution")
    }
    fn syntax(&mut self) -> SshCommandResult {
        panic!("no syntax")
    }
    fn connection(&mut self, spec: &str) -> SshCommandResult {
        self.specs.push(spec.into());
        self.answers.pop_front().unwrap()
    }
}
fn fixture() -> (Files, Ssh) {
    (
        Files {
            exists: true,
            refuse: false,
            pretend: false,
            calls: vec![],
        },
        Ssh {
            available: true,
            answers: VecDeque::from([
                completed("passwordauthentication no\nkbdinteractiveauthentication yes\n"),
                completed("passwordauthentication yes\nkbdinteractiveauthentication no\n"),
            ]),
            specs: vec![],
        },
    )
}
fn run(
    files: &mut Files,
    ssh: &mut Ssh,
    user: Option<&str>,
    allow_missing: bool,
) -> (u8, String, String) {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let lookup = || user.map(str::to_owned);
    let context = SshVerifyContext {
        user: &lookup,
        allow_missing,
        executable: Path::new("/fixture/sshd"),
    };
    let status = rollback_ssh(
        files,
        ssh,
        &context,
        &mut SshOutput {
            stdout: &mut out,
            stderr: &mut err,
        },
    );
    (
        status,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}
#[test]
fn removal_precedes_both_password_samples_and_never_restarts() {
    let (mut files, mut ssh) = fixture();
    let (status, out, err) = run(&mut files, &mut ssh, Some("operator"), false);
    assert_eq!(status, 0, "{err}");
    assert!(!files.exists);
    assert_eq!(files.calls, ["prime", "remove"]);
    assert_eq!(
        ssh.specs,
        [
            "user=operator,host=localhost,addr=127.0.0.1",
            "user=operator,host=recovery.invalid,addr=198.51.100.23"
        ]
    );
    assert!(out.contains("at the next sshd start"));
    assert!(out.contains("running daemon keeps"));
}
#[test]
fn an_already_absent_target_still_requires_recovery_proof() {
    let (mut files, mut ssh) = fixture();
    files.exists = false;
    let (status, out, err) = run(&mut files, &mut ssh, Some("operator"), false);
    assert_eq!(status, 0, "{err}");
    assert!(out.contains("already absent"));
    assert!(files.calls.is_empty());
    assert_eq!(ssh.specs.len(), 2);
}
#[test]
fn failed_or_dishonest_removal_stops_before_claiming_recovery() {
    for pretend in [false, true] {
        let (mut files, mut ssh) = fixture();
        files.pretend = pretend;
        files.refuse = !pretend;
        let (status, out, err) = run(&mut files, &mut ssh, Some("operator"), false);
        assert_eq!(status, 1);
        assert!(!err.is_empty());
        assert!(files.exists);
        assert!(ssh.specs.is_empty());
        assert!(!out.contains("complete"));
    }
}
#[test]
fn blocked_and_unreadable_channels_fail_distinctly_after_removal() {
    for (answer, word) in [
        (
            completed("passwordauthentication no\nkbdinteractiveauthentication no\n"),
            "NOT restored",
        ),
        (completed("passwordauthentication yes\n"), "errored"),
        (Err(InspectionFailure::TimedOut), "errored"),
        (
            Ok(SshCompleted {
                status: 255,
                output: vec![],
            }),
            "errored",
        ),
    ] {
        let (mut files, mut ssh) = fixture();
        ssh.answers[1] = answer;
        let (status, out, err) = run(&mut files, &mut ssh, Some("operator"), false);
        assert_eq!(status, 1);
        assert!(err.contains(word), "{err}");
        assert!(!files.exists);
        assert!(!out.contains("complete"));
    }
}
#[test]
fn unavailable_binary_or_identity_never_claims_an_open_channel() {
    for allow in [false, true] {
        let (mut files, mut ssh) = fixture();
        ssh.available = false;
        let (status, out, err) = run(&mut files, &mut ssh, Some("operator"), allow);
        assert_eq!(status, if allow { 0 } else { 1 }, "{err}");
        assert!(!files.exists);
        if allow {
            assert!(out.contains("NOT checked"));
        }
        assert!(!out.contains("complete"));
    }
    let (mut files, mut ssh) = fixture();
    let (status, _, err) = run(&mut files, &mut ssh, None, false);
    assert_eq!(status, 1);
    assert!(err.contains("errored"));
    assert!(ssh.specs.is_empty());
    assert!(!files.exists);
}
