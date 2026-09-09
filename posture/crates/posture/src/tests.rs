use super::*;
use posture_application::InspectionFailure;

struct Inspection {
    signature: Result<Vec<u8>, InspectionFailure>,
    paths: Vec<OsString>,
}
impl EnrichmentInspection for Inspection {
    fn plist_value(&mut self, _: &Path, _: &str) -> Result<Vec<u8>, InspectionFailure> {
        panic!("a bundle is assessed directly")
    }
    fn signing(&mut self, path: &Path) -> Result<Vec<u8>, InspectionFailure> {
        self.paths.push(path.as_os_str().to_owned());
        self.signature.clone()
    }
    fn quarantined(&mut self, _: &Path) -> bool {
        false
    }
    fn is_file(&self, _: &Path) -> bool {
        panic!("a bundle needs no file-kind probe")
    }
    fn is_mach_o(&mut self, _: &Path) -> bool {
        panic!("a bundle needs no file-kind probe")
    }
    fn metadata(&mut self, _: &Path) -> Option<Vec<u8>> {
        panic!("a bundle needs no stat context")
    }
}

#[test]
fn the_cli_composes_enrichment_and_preserves_its_fact_and_exit_status() {
    for (signature, status, fact) in [
        (
            Ok(b"Authority=Apple".to_vec()),
            0,
            b"signed: Apple".as_slice(),
        ),
        (Err(InspectionFailure::TimedOut), 10, b"UNSIGNED"),
    ] {
        let mut inspection = Inspection {
            signature,
            paths: Vec::new(),
        };
        let mut out = Vec::new();
        let mut err = Vec::new();
        let path = OsString::from("/a quoted path.app");
        assert_eq!(
            execute(
                &["enrich".into(), path.clone(), "ignored".into()],
                &mut inspection,
                &mut out,
                &mut err
            ),
            status
        );
        assert_eq!(out, fact);
        assert!(err.is_empty());
        assert_eq!(inspection.paths, [path]);
    }
}
