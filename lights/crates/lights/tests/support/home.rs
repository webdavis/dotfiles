use std::path::{Path, PathBuf};

/// A temporary test home directory, removed when dropped.
pub struct Home(PathBuf);

impl Home {
    // A pid IS NOT UNIQUE OVER TIME: macOS recycles them, so a name built from
    // the pid and a counter can match a directory a past run left behind.
    // Clearing first is what makes the name safe to reuse; the `Drop` below
    // is what stops them piling up in the first place.
    pub fn fresh(path: PathBuf) -> Home {
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Home(path)
    }
    pub fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for Home {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[test]
fn fresh_clears_pre_existing_contents() {
    let path = std::env::temp_dir().join(format!("lights-home-fresh-test-{}", std::process::id()));
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(path.join("config.toml"), "stale").unwrap();
    let home = Home::fresh(path);
    assert_eq!(std::fs::read_dir(home.path()).unwrap().count(), 0);
}
