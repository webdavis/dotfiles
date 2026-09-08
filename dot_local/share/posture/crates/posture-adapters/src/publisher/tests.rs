use super::*;
use crate::CommandIo;
use posture_application::InspectionFailure;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Scripted {
    source: PathBuf,
    deployed: PathBuf,
    tree: PathBuf,
    calls: Vec<(PathBuf, Vec<OsString>)>,
    apply: Result<Vec<u8>, InspectionFailure>,
    locate: Result<Vec<u8>, InspectionFailure>,
    manifest: Result<Vec<u8>, InspectionFailure>,
}
impl CommandRunner for Scripted {
    fn run_completed(
        &mut self,
        program: &Path,
        args: &[&OsStr],
        io: CommandIo,
    ) -> Result<crate::CommandOutput, InspectionFailure> {
        assert_eq!(
            fs::read(&self.source).unwrap(),
            b"new source\n",
            "source must precede every publication command"
        );
        self.calls.push((
            program.into(),
            args.iter().map(|s| s.to_os_string()).collect(),
        ));
        if args.first() == Some(&OsStr::new("apply")) {
            assert_eq!(program, Path::new("/fixture/chezmoi"));
            assert_eq!(
                args,
                [
                    OsStr::new("apply"),
                    OsStr::new("--force"),
                    self.deployed.as_os_str()
                ]
            );
            assert_eq!(io, CommandIo::InheritAll);
            // Simulate a partial apply even when it returns failure.
            fs::copy(&self.source, &self.deployed).unwrap();
            self.apply
                .clone()
                .map(|bytes| crate::CommandOutput { bytes, exit: 0 })
        } else if args.first() == Some(&OsStr::new("source-path")) {
            assert_eq!(io, CommandIo::CaptureStdout);
            self.locate
                .clone()
                .map(|bytes| crate::CommandOutput { bytes, exit: 0 })
        } else {
            assert_eq!(io, CommandIo::InheritAll);
            assert_eq!(program, Path::new("/bin/bash"));
            assert_eq!(fs::read(&self.deployed).unwrap(), b"new source\n");
            if self.manifest.is_ok() {
                fs::write(self.tree.join("refreshed"), b"done").unwrap();
            }
            self.manifest
                .clone()
                .map(|bytes| crate::CommandOutput { bytes, exit: 0 })
        }
    }
}
struct Fixture {
    root: PathBuf,
    publisher: AllowlistPublisher,
    runner: Scripted,
}
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "posture-publish-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let tree = root.join("tree");
        fs::create_dir_all(tree.join(".chezmoiscripts")).unwrap();
        fs::write(
            tree.join(".chezmoiscripts/run_after_05-osquery-known-good-manifests.sh"),
            b"inert",
        )
        .unwrap();
        let source = root.join("source");
        let deployed = root.join("deployed");
        fs::write(&source, b"old source\n").unwrap();
        fs::write(&deployed, b"old deployed\n").unwrap();
        let publisher = AllowlistPublisher::new(
            "/fixture/chezmoi".into(),
            deployed.clone(),
            None,
            Duration::from_millis(100),
        );
        let runner = Scripted {
            source,
            deployed,
            tree: tree.clone(),
            calls: vec![],
            apply: Ok(vec![]),
            locate: Ok(format!("{}\n", tree.display()).into_bytes()),
            manifest: Ok(vec![]),
        };
        Self {
            root,
            publisher,
            runner,
        }
    }
    fn publish(&mut self) -> Result<(), PublicationRefusal> {
        let source = self.runner.source.clone();
        self.publisher.publish_with(
            &mut self.runner,
            &source,
            &[CuratedLine::Preserved(b"new source")],
        )
    }
}
#[test]
fn publication_orders_source_apply_location_and_manifest_with_inherited_io() {
    let mut f = Fixture::new();
    assert_eq!(f.publish(), Ok(()));
    assert_eq!(f.runner.calls.len(), 3);
    assert_eq!(f.runner.calls[1].1, [OsString::from("source-path")]);
    assert_eq!(
        f.runner.calls[2].1,
        [f.runner
            .tree
            .join(".chezmoiscripts/run_after_05-osquery-known-good-manifests.sh")
            .into_os_string()]
    );
    assert!(f.runner.tree.join("refreshed").is_file());
}
#[test]
fn apply_failure_or_timeout_restores_source_but_does_not_claim_deployment_was_unchanged() {
    for failure in [InspectionFailure::Failed, InspectionFailure::TimedOut] {
        let mut f = Fixture::new();
        f.runner.apply = Err(failure);
        assert_eq!(
            f.publish(),
            Err(PublicationRefusal::Apply {
                rollback_error: None
            })
        );
        assert_eq!(fs::read(&f.runner.source).unwrap(), b"old source\n");
        assert_eq!(fs::read(&f.runner.deployed).unwrap(), b"new source\n");
        assert_eq!(f.runner.calls.len(), 1);
        assert!(!f.runner.tree.join("refreshed").exists());
    }
}
#[test]
fn manifest_failure_or_timeout_keeps_new_source_and_deployed_bytes() {
    for failure in [InspectionFailure::Failed, InspectionFailure::TimedOut] {
        let mut f = Fixture::new();
        f.runner.manifest = Err(failure);
        assert_eq!(
            f.publish(),
            Err(PublicationRefusal::ManifestRefresh(f.runner.tree.join(
                ".chezmoiscripts/run_after_05-osquery-known-good-manifests.sh"
            )))
        );
        assert_eq!(fs::read(&f.runner.source).unwrap(), b"new source\n");
        assert_eq!(fs::read(&f.runner.deployed).unwrap(), b"new source\n");
        assert_eq!(f.runner.calls.len(), 3);
    }
}
#[test]
fn a_failed_source_directory_lookup_reports_stale_without_attempting_a_manifest() {
    let mut f = Fixture::new();
    f.runner.locate = Err(InspectionFailure::Failed);
    assert_eq!(f.publish(), Err(PublicationRefusal::ManifestMissing(None)));
    assert_eq!(f.runner.calls.len(), 2);
    assert_eq!(fs::read(&f.runner.source).unwrap(), b"new source\n");
}
#[test]
fn an_explicit_missing_manifest_does_not_trigger_a_source_directory_lookup() {
    let mut f = Fixture::new();
    let missing = f.root.join("absent-runner");
    f.publisher.manifest = Some(missing.clone());
    assert_eq!(
        f.publish(),
        Err(PublicationRefusal::ManifestMissing(Some(missing)))
    );
    assert_eq!(f.runner.calls.len(), 1);
    assert_eq!(fs::read(&f.runner.deployed).unwrap(), b"new source\n");
}
#[test]
fn missing_original_source_rolls_back_to_an_empty_file_after_failed_apply() {
    let mut f = Fixture::new();
    f.runner.source = f.root.join("initially-absent");
    f.runner.apply = Err(InspectionFailure::Failed);
    assert_eq!(
        f.publish(),
        Err(PublicationRefusal::Apply {
            rollback_error: None
        })
    );
    assert_eq!(fs::read(&f.runner.source).unwrap(), b"");
}
#[test]
fn a_failed_source_write_never_runs_apply_or_manifest() {
    let mut f = Fixture::new();
    f.runner.source = f.root.join("absent-parent/source");
    assert!(matches!(
        f.publish(),
        Err(PublicationRefusal::SourceWrite(_))
    ));
    assert!(f.runner.calls.is_empty());
    assert_eq!(fs::read(&f.runner.deployed).unwrap(), b"old deployed\n");
}
