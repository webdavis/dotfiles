use super::*;
use crate::CommandIo;
use posture_application::InspectionFailure;
use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Query {
    output: Result<Vec<u8>, InspectionFailure>,
    calls: usize,
}
impl CommandRunner for Query {
    fn run(
        &mut self,
        program: &Path,
        args: &[&OsStr],
        io: CommandIo,
    ) -> Result<Vec<u8>, InspectionFailure> {
        self.calls += 1;
        assert_eq!(program, Path::new("/fixture/osqueryi"));
        assert_eq!(
            args,
            [
                OsStr::new("--json"),
                OsStr::new(
                    "SELECT path, COALESCE(NULLIF(program,''), program_arguments) AS program FROM launchd WHERE label = 'my.agent';"
                )
            ]
        );
        assert_eq!(
            io,
            CommandIo::Inspection {
                merge_stderr: false
            }
        );
        self.output.clone()
    }
}
fn table() -> SystemLaunchdTable {
    SystemLaunchdTable::new("/fixture/osqueryi".into(), Duration::from_millis(100))
}
fn fixture() -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let p = std::env::temp_dir().join(format!(
        "posture-plist-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::write(&p, b"hello world").unwrap();
    p
}
fn query(path: &Path) -> Query {
    Query {
        output: Ok(serde_json::to_vec(
            &serde_json::json!([{"path":path,"program":"/owned program --arg"}]),
        )
        .unwrap()),
        calls: 0,
    }
}
#[test]
fn capture_uses_the_launchd_query_and_hashes_the_exact_first_plist_bytes() {
    let path = fixture();
    let mut runner = query(&path);
    assert_eq!(
        table().capture_with("my.agent", &mut runner),
        Ok(CapturedAgent {
            path: path.to_str().unwrap().into(),
            program: "/owned program --arg".into(),
            sha256: "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9".into()
        })
    );
    assert_eq!(runner.calls, 1);
}
#[test]
fn capture_follows_a_regular_plist_symlink_but_refuses_a_missing_or_nonregular_plist() {
    let path = fixture();
    let link = path.with_extension("link");
    std::os::unix::fs::symlink(&path, &link).unwrap();
    assert!(table().capture_with("my.agent", &mut query(&link)).is_ok());
    for bad in [path.with_extension("missing"), std::env::temp_dir()] {
        assert_eq!(
            table().capture_with("my.agent", &mut query(&bad)),
            Err(CaptureRefusal::Hash(bad.to_str().unwrap().into()))
        );
    }
}
#[test]
fn failed_or_timed_out_queries_are_not_retried_and_cannot_supply_an_identity() {
    for output in [
        Err(InspectionFailure::Failed),
        Err(InspectionFailure::TimedOut),
    ] {
        let mut runner = Query { output, calls: 0 };
        assert_eq!(
            table().capture_with("my.agent", &mut runner),
            Err(CaptureRefusal::NoAgent)
        );
        assert_eq!(runner.calls, 1);
    }
}
#[test]
fn empty_or_invalid_query_fields_refuse_before_hash_capture() {
    for bytes in [
        b"[]".as_slice(),
        b"garbage",
        b"[{\"path\":\"/missing\",\"program\":false}]",
        b"[{\"path\":null,\"program\":\"owned\"}]",
    ] {
        let mut runner = Query {
            output: Ok(bytes.to_vec()),
            calls: 0,
        };
        assert_eq!(
            table().capture_with("my.agent", &mut runner),
            Err(CaptureRefusal::NoAgent)
        );
        assert_eq!(runner.calls, 1);
    }
}
