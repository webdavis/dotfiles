use super::*;
use crate::{DesiredTree, StagingRefusal};
use posture_domain::{ContentComparison, LiveAttributes, LiveEntry, ParentPid};
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::Rc,
    time::Duration,
};

type Calls = Rc<RefCell<Vec<String>>>;
pub(super) fn restarted() -> Restarted {
    Restarted {
        parent: ParentPid::parse("20").unwrap(),
        settled_for: Duration::from_secs(1),
    }
}
pub(super) struct Snapshot(Rc<Cell<bool>>);
impl DesiredTree for Snapshot {
    fn source(&self, file: ConvergeFile) -> PathBuf {
        Path::new("/snapshot").join(file.relative_path())
    }
}
impl Drop for Snapshot {
    fn drop(&mut self) {
        self.0.set(true);
    }
}
pub(super) struct Staging {
    calls: Calls,
    dropped: Rc<Cell<bool>>,
    pub refuse: bool,
}
impl ConvergeStaging for Staging {
    type Prepared = Snapshot;
    fn prepare(&self) -> Result<Snapshot, Vec<StagingRefusal>> {
        self.calls.borrow_mut().push("stage".into());
        if self.refuse {
            return Err(vec![StagingRefusal::MissingDirectory("desired".into())]);
        }
        Ok(Snapshot(self.dropped.clone()))
    }
}
pub(super) struct Live {
    calls: Calls,
    repaired: Rc<Cell<bool>>,
    pub target_drift: bool,
    pub irregular_packs: bool,
    pub changed: Option<ConvergeFile>,
    pub files_require_repair: bool,
}
impl LiveTree for Live {
    fn directory(&mut self, directory: ConvergeDirectory) -> LiveEntry {
        self.calls.borrow_mut().push(format!("probe:{directory:?}"));
        if directory == ConvergeDirectory::Packs && self.irregular_packs {
            return LiveEntry::Irregular;
        }
        let mode = if directory == ConvergeDirectory::Target && self.target_drift {
            0o777
        } else {
            0o755
        };
        LiveEntry::Directory(LiveAttributes {
            mode,
            uid: 0,
            gid: 0,
        })
    }
    fn file(&mut self, file: ConvergeFile, source: &Path) -> (LiveEntry, ContentComparison) {
        self.calls
            .borrow_mut()
            .push(format!("file:{}", source.display()));
        if self.changed == Some(file) || self.files_require_repair && !self.repaired.get() {
            return (LiveEntry::Absent, ContentComparison::Unreadable);
        }
        (
            LiveEntry::File(LiveAttributes {
                mode: 0o644,
                uid: 0,
                gid: 0,
            }),
            ContentComparison::Equal,
        )
    }
}
pub(super) struct Install {
    calls: Calls,
    repaired: Rc<Cell<bool>>,
    dropped: Rc<Cell<bool>>,
    pub reject: Option<&'static str>,
    pub create_log: bool,
}
impl Install {
    fn result(&self, operation: &str) -> Result<(), InspectionFailure> {
        assert!(
            !self.dropped.get(),
            "snapshot released before root finished reading it"
        );
        if self.reject == Some(operation) {
            Err(InspectionFailure::Failed)
        } else {
            Ok(())
        }
    }
}
impl PrivilegedInstall for Install {
    fn directory(&mut self, directory: ConvergeDirectory) -> Result<(), InspectionFailure> {
        self.calls.borrow_mut().push(format!("write:{directory:?}"));
        self.result("directory")?;
        self.repaired.set(true);
        Ok(())
    }
    fn file(&mut self, file: ConvergeFile, source: &Path) -> Result<(), InspectionFailure> {
        self.calls.borrow_mut().push(format!(
            "write:{}:{}",
            file.relative_path(),
            source.display()
        ));
        self.result("file")
    }
    fn log_directory(&mut self) -> Result<bool, InspectionFailure> {
        self.calls.borrow_mut().push("log".into());
        self.result("log")?;
        Ok(self.create_log)
    }
}
pub(super) struct Fixture {
    pub calls: Calls,
    pub dropped: Rc<Cell<bool>>,
    pub events: Rc<RefCell<Vec<ConvergeEvent>>>,
    pub staging: Staging,
    pub live: Live,
    pub install: Install,
    pub reject_report: bool,
    pub reject_restart: bool,
}
impl Fixture {
    pub fn new() -> Self {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let dropped = Rc::new(Cell::new(false));
        let repaired = Rc::new(Cell::new(false));
        Self {
            calls: calls.clone(),
            dropped: dropped.clone(),
            events: Rc::new(RefCell::new(Vec::new())),
            staging: Staging {
                calls: calls.clone(),
                dropped: dropped.clone(),
                refuse: false,
            },
            live: Live {
                calls: calls.clone(),
                repaired: repaired.clone(),
                target_drift: false,
                irregular_packs: false,
                changed: None,
                files_require_repair: false,
            },
            install: Install {
                calls,
                repaired,
                dropped,
                reject: None,
                create_log: false,
            },
            reject_report: false,
            reject_restart: false,
        }
    }
    pub fn run(&mut self) -> Result<(), ConvergeFailure> {
        converge(
            &self.staging,
            &mut self.live,
            &mut self.install,
            || {
                assert!(!self.dropped.get());
                self.calls.borrow_mut().push("restart".into());
                if self.reject_restart {
                    Err(RestartFailure::Deadline)
                } else {
                    Ok(restarted())
                }
            },
            |event| {
                assert!(!self.dropped.get());
                if self.reject_report {
                    return Err(io::Error::other("closed output"));
                }
                self.events.borrow_mut().push(event);
                Ok(())
            },
        )
    }
    pub fn writes(&self) -> Vec<String> {
        self.calls
            .borrow()
            .iter()
            .filter(|call| call.starts_with("write:"))
            .cloned()
            .collect()
    }
}
