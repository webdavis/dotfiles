use super::*;
use std::collections::HashMap;
use std::path::PathBuf;

struct Fixture {
    plist: HashMap<&'static str, Result<Vec<u8>, InspectionFailure>>,
    signature: Result<Vec<u8>, InspectionFailure>,
    downloaded: bool,
    mach_o: bool,
    context: Option<Vec<u8>>,
    calls: Vec<String>,
}
impl Default for Fixture {
    fn default() -> Self {
        Self {
            plist: HashMap::new(),
            signature: Ok(b"Authority=Apple Root\n".to_vec()),
            downloaded: false,
            mach_o: true,
            context: Some(
                b"owner fixture, mode -rw-r--r--, modified 2026-09-07T00:00:00Z".to_vec(),
            ),
            calls: Vec::new(),
        }
    }
}
impl EnrichmentInspection for Fixture {
    fn plist_value(&mut self, path: &Path, key: &str) -> Result<Vec<u8>, InspectionFailure> {
        self.calls.push(format!("plutil:{key}:{}", path.display()));
        self.plist
            .get(key)
            .cloned()
            .unwrap_or(Err(InspectionFailure::Failed))
    }
    fn signing(&mut self, path: &Path) -> Result<Vec<u8>, InspectionFailure> {
        self.calls.push(format!("codesign:{}", path.display()));
        self.signature.clone()
    }
    fn quarantined(&mut self, path: &Path) -> bool {
        self.calls.push(format!("xattr:{}", path.display()));
        self.downloaded
    }
    fn is_file(&self, path: &Path) -> bool {
        matches!(
            path.to_str(),
            Some("/fixture/a quoted script.sh" | "/fixture/plain binary")
        )
    }
    fn is_mach_o(&mut self, path: &Path) -> bool {
        if !self.is_file(path) {
            return false;
        }
        self.calls.push(format!("file:{}", path.display()));
        self.mach_o
    }
    fn metadata(&mut self, path: &Path) -> Option<Vec<u8>> {
        if !self.is_file(path) && path != Path::new("/fixture/directory") {
            return None;
        }
        self.calls.push(format!("stat:{}", path.display()));
        self.context.clone()
    }
}
fn check(path: &str, fixture: &mut Fixture, fact: &[u8], status: u8, calls: &[&str]) {
    let result = enrich(&PathBuf::from(path), fixture);
    assert_eq!(result.fact, fact, "the Bash fact bytes must survive");
    assert_eq!(
        result.exit_code(),
        status,
        "the caller routes on the exact exit status"
    );
    assert_eq!(
        fixture.calls, calls,
        "inspection order and selected path must survive"
    );
}

mod interpreters;
mod ordinary;
mod resolution;
mod signing;
