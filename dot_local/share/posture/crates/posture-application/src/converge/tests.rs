use super::*;
use posture_domain::LiveAttributes;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[derive(Debug)]
struct Snapshot(Rc<Cell<bool>>);
impl DesiredTree for Snapshot {
    fn source(&self, file: ConvergeFile) -> PathBuf {
        Path::new("/private-owned-snapshot").join(file.relative_path())
    }
}
impl Drop for Snapshot {
    fn drop(&mut self) {
        self.0.set(true);
    }
}
struct Staging {
    calls: Rc<RefCell<Vec<String>>>,
    dropped: Rc<Cell<bool>>,
    refuse: bool,
}
impl ConvergeStaging for Staging {
    type Prepared = Snapshot;
    fn prepare(&self) -> Result<Snapshot, Vec<StagingRefusal>> {
        self.calls.borrow_mut().push("prepare".into());
        if self.refuse {
            return Err(vec![StagingRefusal::MissingDirectory("desired".into())]);
        }
        Ok(Snapshot(self.dropped.clone()))
    }
}
struct Live {
    calls: Rc<RefCell<Vec<String>>>,
    irregular: Option<ConvergeDirectory>,
    changed: Option<ConvergeFile>,
}
impl LiveTree for Live {
    fn directory(&mut self, directory: ConvergeDirectory) -> LiveEntry {
        self.calls
            .borrow_mut()
            .push(format!("directory:{directory:?}"));
        if self.irregular == Some(directory) {
            return LiveEntry::Irregular;
        }
        LiveEntry::Directory(LiveAttributes {
            mode: 0o755,
            uid: 0,
            gid: 0,
        })
    }
    fn file(&mut self, file: ConvergeFile, desired: &Path) -> (LiveEntry, ContentComparison) {
        self.calls
            .borrow_mut()
            .push(format!("file:{}", desired.display()));
        (
            LiveEntry::File(LiveAttributes {
                mode: 0o644,
                uid: 0,
                gid: 0,
            }),
            if self.changed == Some(file) {
                ContentComparison::Different
            } else {
                ContentComparison::Equal
            },
        )
    }
}
fn fixture() -> (Staging, Live) {
    let calls = Rc::new(RefCell::new(Vec::new()));
    (
        Staging {
            calls: calls.clone(),
            dropped: Rc::new(Cell::new(false)),
            refuse: false,
        },
        Live {
            calls,
            irregular: None,
            changed: None,
        },
    )
}

#[test]
fn staging_refusal_prevents_every_live_probe() {
    let (mut staging, mut live) = fixture();
    staging.refuse = true;
    assert_eq!(
        prepare_converge(&staging, &mut live).unwrap_err(),
        ConvergeRefusal::Staging(vec![StagingRefusal::MissingDirectory("desired".into())])
    );
    assert_eq!(*staging.calls.borrow(), ["prepare"]);
}

#[test]
fn both_directory_verdicts_precede_an_irregular_directory_refusal() {
    for directory in ConvergeDirectory::ALL {
        let (staging, mut live) = fixture();
        live.irregular = Some(directory);
        assert_eq!(
            prepare_converge(&staging, &mut live).unwrap_err(),
            ConvergeRefusal::IrregularDirectory(directory)
        );
        assert_eq!(
            *staging.calls.borrow(),
            ["prepare", "directory:Target", "directory:Packs"]
        );
        assert!(staging.dropped.get());
    }
}

#[test]
fn the_plan_reads_only_staged_sources_and_retains_the_snapshot() {
    let (staging, mut live) = fixture();
    let plan = prepare_converge(&staging, &mut live).unwrap();
    let calls = staging.calls.borrow();
    assert_eq!(
        &calls[..3],
        ["prepare", "directory:Target", "directory:Packs"]
    );
    let sources: Vec<_> = [
        "osquery.conf",
        "osquery.flags",
        "packs/agent-attack-surface.conf",
        "packs/installed-software-drift.conf",
        "packs/intrusion-detection.conf",
        "packs/security-policy-regression.conf",
    ]
    .map(|path| format!("file:/private-owned-snapshot/{path}"))
    .into();
    assert_eq!(&calls[3..], sources);
    assert!(!staging.dropped.get());
    assert!(!plan.needs_restart());
    drop(plan);
    assert!(staging.dropped.get());
}

#[test]
fn only_drifted_paths_are_planned_and_the_last_file_can_require_restart() {
    let (staging, mut live) = fixture();
    live.changed = Some(ConvergeFile::SecurityPolicyRegression);
    let plan = prepare_converge(&staging, &mut live).unwrap();
    assert_eq!(
        plan.directories,
        [
            (ConvergeDirectory::Target, Drift::Ok),
            (ConvergeDirectory::Packs, Drift::Ok)
        ]
    );
    let repairs: Vec<_> = plan
        .files
        .into_iter()
        .filter(|(_, verdict)| *verdict != Drift::Ok)
        .collect();
    assert_eq!(
        repairs,
        [(ConvergeFile::SecurityPolicyRegression, Drift::Content)]
    );
    assert!(plan.needs_restart());
    assert_eq!(staging.calls.borrow().len(), 9);
}
