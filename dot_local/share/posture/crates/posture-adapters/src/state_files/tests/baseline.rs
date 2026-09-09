use super::*;
use posture_domain::{ControlRecord, ControlsInput, validate_controls};
fn controls() -> Vec<Control> {
    validate_controls(ControlsInput::Records(&[ControlRecord {
        id: "old",
        tier: "verify",
        reader: "fdesetup_status",
        expect: "on",
        target: "",
        description: "old",
        remedy: "",
    }]))
    .unwrap()
}
fn cases() -> Vec<serde_json::Value> {
    serde_json::from_str(include_str!("baseline.json")).unwrap()
}
fn path(case: &serde_json::Value) -> PathBuf {
    let dir = root();
    let path = dir.join("state");
    let name = case["name"].as_str().unwrap();
    if name == "directory" {
        fs::create_dir(&path).unwrap();
    } else if name == "fifo" {
        use std::os::unix::ffi::OsStrExt;
        let name = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    } else if name == "symlink" {
        let target = dir.join("target");
        put(&target, case["input"].as_str().unwrap(), 0o600);
        std::os::unix::fs::symlink(target, &path).unwrap();
    } else if let Some(raw) = case["input"].as_str() {
        put(&path, raw, case["mode"].as_u64().unwrap() as u32);
    }
    path
}
#[test]
fn a_baseline_read_preserves_captured_scalars_and_control_declaration_fields() {
    for case in cases().into_iter().filter(|c| c["fields"][0] == "1") {
        let mut store = PollStateFiles::new(path(&case));
        let reading = store.read(&controls()).unwrap();
        assert_eq!(store.prior_json.as_deref(), case["fields"][5].as_str());
        let wanted: Vec<_> = case["fields"].as_array().unwrap()[2..5]
            .iter()
            .map(|x| x.as_str().unwrap())
            .collect();
        assert_eq!(
            reading.values.each_ref().map(String::as_str).as_slice(),
            wanted,
            "{}",
            case["name"]
        );
        let priors: Vec<_> = reading.controls.iter().map(|c| c.prior()).collect();
        assert!(reading.baseline(&priors).is_some());
        let object: serde_json::Value =
            serde_json::from_str(case["fields"][5].as_str().unwrap()).unwrap();
        assert_eq!(
            reading.controls[0].value,
            object["old"].as_str().unwrap_or("")
        );
        assert_eq!(
            reading.controls[0].expect,
            object["old:expect"].as_str().unwrap_or("")
        );
        assert_eq!(
            reading.controls[0].target,
            object["old:target"].as_str().unwrap_or("")
        );
        if case["name"] == "valid" {
            assert_eq!(reading.controls[0].id, "old");
            assert_eq!(reading.controls[0].value, "off");
            assert_eq!(reading.controls[0].expect, "on");
            assert_eq!(reading.controls[0].target, "");
        }
    }
}
#[test]
fn baseline_trust_refuses_wrong_modes_shapes_and_symlink_own_modes_without_blocking() {
    for case in cases().into_iter().filter(|c| c["fields"][0] != "1") {
        let path = path(&case);
        let mut store = PollStateFiles::new(path.clone());
        assert!(store.read(&controls()).is_none(), "{}", case["name"]);
        if case["name"] == "symlink" {
            assert!(path.is_symlink());
            assert_eq!(modes(&fs::read_link(&path).unwrap()), 0o600);
            assert_ne!(modes(&path), 0o600);
        }
    }
}
