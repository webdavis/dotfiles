use super::*;
use crate::{InspectionFailure, SshCommandResult, SshCompleted, SshFile};
use posture_domain::ssh_config;
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    path::PathBuf,
    rc::Rc,
};
type Trace = Rc<RefCell<Vec<String>>>;
struct Files {
    entries: BTreeMap<String, Vec<u8>>,
    directory: bool,
    calls: Trace,
    fail: Option<&'static str>,
    published_failure: bool,
    signal_at: Option<&'static str>,
    pending: Rc<Cell<Option<i32>>>,
}
impl Files {
    fn step(&mut self, name: &str) -> SshCommandResult {
        self.calls.borrow_mut().push(name.into());
        if self.signal_at == Some(name) {
            self.pending.set(Some(15));
        }
        if self.fail == Some(name) {
            Err(InspectionFailure::TimedOut)
        } else {
            Ok(SshCompleted {
                status: 0,
                output: vec![],
            })
        }
    }
    fn has(&self, file: SshFile) -> bool {
        self.entries.contains_key(file.name())
    }
}
impl SshInstallFiles for Files {
    fn path(&self, file: SshFile) -> PathBuf {
        PathBuf::from("/fixture").join(file.name())
    }
    fn directory_exists(&self) -> bool {
        self.directory
    }
    fn exists(&self, file: SshFile) -> Result<bool, InspectionFailure> {
        Ok(self.has(file))
    }
    fn prime(&mut self) -> SshCommandResult {
        self.step("prime")
    }
    fn stage(&mut self, bytes: &[u8]) -> SshCommandResult {
        self.entries
            .insert(SshFile::Staging.name().into(), bytes.into());
        self.step("stage")
    }
    fn chmod(&mut self) -> SshCommandResult {
        self.step("chmod")
    }
    fn save(&mut self) -> SshCommandResult {
        let result = self.step("save");
        if super::super::succeeded(&result) {
            let bytes = self.entries[SshFile::Target.name()].clone();
            self.entries
                .insert(SshFile::SavedTarget.name().into(), bytes);
        }
        result
    }
    fn rename(&mut self, from: SshFile, to: SshFile) -> SshCommandResult {
        let name = match (from, to) {
            (SshFile::Staging, SshFile::Target) => "publish",
            (SshFile::Legacy, SshFile::SavedLegacy) => "retire",
            (SshFile::SavedTarget, SshFile::Target) => "restore-target",
            (SshFile::SavedLegacy, SshFile::Legacy) => "restore-legacy",
            _ => panic!("unexpected rename"),
        };
        let result = self.step(name);
        if super::super::succeeded(&result) {
            let bytes = self.entries.remove(from.name()).unwrap();
            self.entries.insert(to.name().into(), bytes);
        }
        if name == "publish" && self.published_failure {
            return Err(InspectionFailure::Failed);
        }
        result
    }
    fn remove(&mut self, files: &[SshFile]) -> SshCommandResult {
        let name = match files {
            [SshFile::Staging, SshFile::SavedTarget, SshFile::SavedLegacy] => "clear",
            [SshFile::SavedTarget, SshFile::SavedLegacy] => "cleanup",
            [SshFile::Staging] => "remove-stage",
            [SshFile::Target] => "remove-target",
            _ => panic!("unexpected removal"),
        };
        let result = self.step(name);
        if super::super::succeeded(&result) {
            for file in files {
                self.entries.remove(file.name());
            }
        }
        result
    }
}
struct Signals {
    calls: Trace,
    pending: Rc<Cell<Option<i32>>>,
}
impl SshInstallSignals for Signals {
    fn pending(&self) -> Option<i32> {
        self.pending.get()
    }
    fn defer(&mut self) {
        self.calls.borrow_mut().push("defer".into());
    }
    fn disarm(&mut self) {
        self.calls.borrow_mut().push("disarm".into());
    }
}
fn fixture(old: bool) -> (Files, Signals) {
    let calls = Rc::new(RefCell::new(vec![]));
    let pending = Rc::new(Cell::new(None));
    let mut entries = BTreeMap::new();
    if old {
        entries.insert(SshFile::Target.name().into(), b"old target".to_vec());
        entries.insert(SshFile::Legacy.name().into(), b"old legacy".to_vec());
    }
    (
        Files {
            entries,
            directory: true,
            calls: calls.clone(),
            fail: None,
            published_failure: false,
            signal_at: None,
            pending: pending.clone(),
        },
        Signals { calls, pending },
    )
}
fn run(files: &mut Files, signals: &mut Signals, result: SshVerification) -> (u8, String, String) {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let trace = files.calls.clone();
    let mut result = Some(result);
    let mut verify = || {
        trace.borrow_mut().push("verify".into());
        result.take().unwrap()
    };
    let code = install_ssh(
        files,
        &mut verify,
        signals,
        &mut SshOutput {
            stdout: &mut out,
            stderr: &mut err,
        },
    );
    (
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}
#[test]
fn install_stages_saves_publishes_retires_verifies_then_disarms_before_cleanup() {
    let (mut f, mut s) = fixture(true);
    let (status, out, err) = run(&mut f, &mut s, SshVerification::Verified);
    assert_eq!(status, 0, "{err}");
    assert_eq!(
        f.entries,
        BTreeMap::from([(
            SshFile::Target.name().into(),
            ssh_config().as_bytes().to_vec()
        )])
    );
    assert_eq!(
        *f.calls.borrow(),
        [
            "prime", "clear", "stage", "chmod", "save", "publish", "retire", "verify", "disarm",
            "cleanup"
        ]
    );
    assert!(out.contains("install complete"));
}

#[test]
fn publication_that_takes_effect_before_reporting_failure_still_restores_the_old_state() {
    for old in [false, true] {
        let (mut f, mut s) = fixture(old);
        f.published_failure = true;
        let (status, out, _) = run(&mut f, &mut s, SshVerification::Verified);
        assert_eq!(status, 1);
        assert!(!out.contains("complete"));
        if old {
            assert_eq!(f.entries[SshFile::Target.name()], b"old target");
        } else {
            assert!(!f.has(SshFile::Target));
        }
    }
}
#[test]
fn failures_before_and_after_publication_restore_existing_targets_and_legacy() {
    for step in [
        "prime", "clear", "stage", "chmod", "save", "publish", "retire",
    ] {
        let (mut f, mut s) = fixture(true);
        f.fail = Some(step);
        let (status, out, err) = run(&mut f, &mut s, SshVerification::Verified);
        assert_eq!(status, 1, "{step}");
        assert!(!err.is_empty());
        assert!(!out.contains("install complete"));
        assert_eq!(f.entries[SshFile::Target.name()], b"old target", "{step}");
        assert_eq!(f.entries[SshFile::Legacy.name()], b"old legacy", "{step}");
        assert!(!f.has(SshFile::Staging), "{step}");
        assert!(!f.calls.borrow().contains(&"verify".into()));
    }
}
#[test]
fn failed_verification_restores_the_old_tree_or_removes_only_the_new_target() {
    for old in [false, true] {
        let (mut f, mut s) = fixture(old);
        let (status, out, err) = run(
            &mut f,
            &mut s,
            SshVerification::Failed(vec!["bad fixture policy".into()]),
        );
        assert_eq!(status, 1);
        assert!(err.contains("bad fixture policy"));
        assert!(!out.contains("complete"));
        if old {
            assert_eq!(f.entries[SshFile::Target.name()], b"old target");
            assert_eq!(f.entries[SshFile::Legacy.name()], b"old legacy");
        } else {
            assert!(f.entries.is_empty());
        }
    }
}
#[test]
fn successful_verification_keeps_installation_when_saved_copy_cleanup_fails() {
    let (mut f, mut s) = fixture(true);
    f.fail = Some("cleanup");
    let (status, out, err) = run(&mut f, &mut s, SshVerification::Verified);
    assert_eq!(status, 0);
    assert!(out.contains("install complete"));
    assert!(err.contains("WARNING"));
    assert_eq!(f.entries[SshFile::Target.name()], ssh_config().as_bytes());
    assert!(f.has(SshFile::SavedTarget));
    assert!(f.has(SshFile::SavedLegacy));
    let trace = f.calls.borrow();
    assert!(trace.iter().position(|s| s == "disarm") < trace.iter().position(|s| s == "cleanup"));
    assert!(!trace.contains(&"restore-target".into()));
}
#[test]
fn explicit_skip_installs_but_never_claims_the_configuration_is_verified() {
    let (mut f, mut s) = fixture(false);
    let (status, out, err) = run(&mut f, &mut s, SshVerification::Skipped);
    assert_eq!(status, 0, "{err}");
    assert!(out.contains("NOT verified"));
    assert!(!out.contains("install complete"));
    assert!(f.has(SshFile::Target));
}
#[test]
fn an_interrupt_at_each_boundary_defers_further_interrupts_and_rolls_back_once() {
    for step in ["clear", "stage", "chmod", "save", "publish", "retire"] {
        let (mut f, mut s) = fixture(true);
        f.signal_at = Some(step);
        let (status, out, err) = run(&mut f, &mut s, SshVerification::Verified);
        assert_eq!(status, 1, "{step}");
        assert!(err.contains("INTERRUPTED"), "{step}: {err}");
        assert!(!out.contains("complete"));
        assert_eq!(f.entries[SshFile::Target.name()], b"old target", "{step}");
        assert_eq!(f.entries[SshFile::Legacy.name()], b"old legacy", "{step}");
        assert_eq!(
            f.calls
                .borrow()
                .iter()
                .filter(|s| s.as_str() == "defer")
                .count(),
            1
        );
        assert!(
            f.calls
                .borrow()
                .iter()
                .filter(|s| s.as_str() == "restore-target")
                .count()
                <= 1
        );
    }
}
#[test]
fn rollback_reports_each_restore_failure_and_still_attempts_the_other_restore() {
    let (mut f, mut s) = fixture(true);
    f.fail = Some("restore-target");
    let (status, _, err) = run(
        &mut f,
        &mut s,
        SshVerification::Failed(vec!["bad policy".into()]),
    );
    assert_eq!(status, 1);
    assert!(err.contains("could not restore"));
    assert!(f.calls.borrow().contains(&"restore-legacy".into()));
    assert_eq!(f.entries[SshFile::Legacy.name()], b"old legacy");
}
#[test]
fn a_missing_directory_refuses_before_privilege_or_writes() {
    let (mut f, mut s) = fixture(true);
    f.directory = false;
    let (status, _, err) = run(&mut f, &mut s, SshVerification::Verified);
    assert_eq!(status, 1);
    assert!(err.contains("directory"));
    assert!(f.calls.borrow().is_empty());
}
