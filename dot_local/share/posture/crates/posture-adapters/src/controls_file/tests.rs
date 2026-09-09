use super::*;
use posture_domain::{ControlReader, ControlValue, ControlsRefusalKind};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

fn directory() -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "posture-controls-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&path).unwrap();
    path
}
fn fixture(bytes: &[u8]) -> PathBuf {
    let path = directory().join("controls.json");
    fs::write(&path, bytes).unwrap();
    path
}
fn captures() -> Vec<serde_json::Value> {
    serde_json::from_str(include_str!("captures.json")).unwrap()
}
fn bytes(capture: &serde_json::Value) -> Vec<u8> {
    capture["input_hex"]
        .as_str()
        .unwrap()
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}
fn valid() -> Vec<u8> {
    bytes(&captures()[0])
}

#[test]
fn controls_files_match_bash_valid_scalar_and_compound_field_bytes() {
    for capture in captures()
        .into_iter()
        .filter(|c| c["stdout_fields"][0] == "")
    {
        let fields = capture["stdout_fields"].as_array().unwrap();
        let controls = read_controls(&fixture(&bytes(&capture)))
            .unwrap_or_else(|error| panic!("{}: {error:?}", capture["name"]));
        assert_eq!(
            controls.len(),
            (fields.len() - 1) / 6,
            "{}",
            capture["name"]
        );
        for (control, expected) in controls.iter().zip(fields[1..].as_chunks::<6>().0) {
            assert_eq!(
                control.id(),
                expected[0].as_str().unwrap(),
                "{}",
                capture["name"]
            );
            assert_eq!(
                Some(control.reader()),
                ControlReader::parse(expected[1].as_str().unwrap()),
                "{}",
                capture["name"]
            );
            assert_eq!(
                Some(control.expect()),
                ControlValue::parse(expected[2].as_str().unwrap()),
                "{}",
                capture["name"]
            );
            assert_eq!(
                control.target(),
                expected[3].as_str().unwrap(),
                "{}",
                capture["name"]
            );
            assert_eq!(
                control.description(),
                expected[4].as_str().unwrap(),
                "{}",
                capture["name"]
            );
            assert_eq!(
                control.remedy(),
                expected[5].as_str().unwrap(),
                "{}",
                capture["name"]
            );
        }
    }
}

#[test]
fn controls_files_refuse_every_captured_invalid_document_without_partial_records() {
    for capture in captures()
        .into_iter()
        .filter(|c| c["stdout_fields"][0] != "")
    {
        let error = read_controls(&fixture(&bytes(&capture))).unwrap_err();
        assert_eq!(
            error.explanation,
            capture["stdout_fields"][0].as_str().unwrap(),
            "{}",
            capture["name"]
        );
    }
}

#[test]
fn controls_files_report_missing_kinds_and_read_refusal_without_blocking() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let root = directory();
    let missing = root.join("absent");
    let broken = root.join("broken");
    symlink(&missing, &broken).unwrap();
    let fifo = root.join("fifo");
    let name = std::ffi::CString::new(fifo.as_os_str().as_encoded_bytes()).unwrap();
    // SAFETY: the C string names only this test's private, absent path.
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    for path in [&missing, &root, &broken, &fifo] {
        let error = read_controls(path).unwrap_err();
        assert_eq!(error.kind, ControlsRefusalKind::Missing);
        assert_eq!(
            error.explanation,
            format!("posture-controls file missing at `{}`", path.display())
        );
    }
    let unreadable = fixture(&valid());
    fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o000)).unwrap();
    let error = read_controls(&unreadable).unwrap_err();
    assert_eq!(error.kind, ControlsRefusalKind::Malformed);
    assert_eq!(
        error.explanation,
        "the posture-controls file is not a JSON array"
    );
}

#[test]
fn controls_files_follow_a_regular_symlink_without_rewriting_its_target() {
    let bytes = valid();
    let target = fixture(&bytes);
    let link = directory().join("controls.json");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let controls = read_controls(&link).unwrap();
    assert_eq!(controls.len(), 1);
    assert_eq!(controls[0].id(), "filevault");
    assert_eq!(fs::read(target).unwrap(), bytes);
    assert!(fs::symlink_metadata(link).unwrap().file_type().is_symlink());
}
