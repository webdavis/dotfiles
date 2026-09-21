use super::*;
use crate::test_sandbox::Sandbox;

pub(super) struct Fixture {
    /// Canonical, because the tree reports the paths it resolved and a
    /// symlinked temporary directory would not match them.
    pub root: PathBuf,
    pub desired: PathBuf,
    pub scratch: PathBuf,
    pub target: PathBuf,
    /// Removes the tree when the test drops the fixture.
    _sandbox: Sandbox,
}
impl Fixture {
    pub fn new() -> Self {
        let sandbox = Sandbox::new("live-tree");
        let root = sandbox.path().canonicalize().unwrap();
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
            _sandbox: sandbox,
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
