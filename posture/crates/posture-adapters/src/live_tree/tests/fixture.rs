use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

pub(super) struct Fixture {
    pub root: PathBuf,
    pub desired: PathBuf,
    pub scratch: PathBuf,
    pub target: PathBuf,
}
impl Fixture {
    pub fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "posture-live-tree-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let desired = root.join("desired");
        let scratch = root.join("scratch");
        let target = root.join("target");
        fs::create_dir_all(desired.join("packs")).unwrap();
        fs::create_dir(&scratch).unwrap();
        fs::create_dir_all(target.join("packs")).unwrap();
        for file in ConvergeFile::ALL {
            fs::write(
                desired.join(file.relative_path()),
                b"desired bytes
",
            )
            .unwrap();
        }
        Self {
            root,
            desired,
            scratch,
            target,
        }
    }
    pub fn staging(&self) -> DesiredStaging {
        DesiredStaging::new(self.desired.clone(), self.scratch.clone())
    }
    pub fn live(&self) -> InstalledTree {
        InstalledTree::new(self.target.clone())
    }
    pub fn config(&self) -> PathBuf {
        self.target.join("osquery.conf")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}
