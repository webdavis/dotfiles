use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

pub(super) const FILES: [(&str, &[u8]); 6] = [
    ("osquery.conf", b"configuration\n"),
    ("osquery.flags", b"flags\n"),
    ("packs/agent-attack-surface.conf", b"agent\n"),
    ("packs/installed-software-drift.conf", b"software\n"),
    ("packs/intrusion-detection.conf", b"intrusion\n"),
    ("packs/security-policy-regression.conf", b"policy\n"),
];

pub(super) struct Fixture {
    pub root: PathBuf,
    pub desired: PathBuf,
    pub scratch: PathBuf,
}

impl Fixture {
    pub fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "posture-staging-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let desired = root.join("desired");
        fs::create_dir(&desired).unwrap();
        fs::create_dir(desired.join("packs")).unwrap();
        let scratch = root.join("scratch");
        fs::create_dir(&scratch).unwrap();
        let fixture = Self {
            root,
            desired,
            scratch,
        };
        fs::write(fixture.neighbor(), b"unrelated").unwrap();
        for (path, bytes) in FILES {
            fs::write(fixture.desired.join(path), bytes).unwrap();
        }
        fixture
    }

    pub fn prepare(&self) -> Result<StagedTree, Vec<StagingRefusal>> {
        DesiredStaging::new(self.desired.clone(), self.scratch.clone()).prepare()
    }

    pub fn neighbor(&self) -> PathBuf {
        self.scratch.join("unrelated")
    }

    pub fn assert_scratch_untouched(&self) {
        assert_eq!(fs::read_dir(&self.scratch).unwrap().count(), 1);
        assert_eq!(fs::read(self.neighbor()).unwrap(), b"unrelated");
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}
