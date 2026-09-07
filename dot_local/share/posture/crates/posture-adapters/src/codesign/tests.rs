use super::*;
use std::ffi::{OsStr, OsString};

#[derive(Default)]
struct Scripted {
    next: Vec<u8>,
    calls: Vec<(String, Vec<OsString>, bool)>,
}
impl CommandRunner for Scripted {
    fn run(
        &mut self,
        program: &Path,
        args: &[&OsStr],
        io: CommandIo,
    ) -> Result<Vec<u8>, InspectionFailure> {
        let CommandIo::Inspection {
            merge_stderr: merged,
        } = io
        else {
            panic!("inspection changed I/O mode")
        };
        self.calls.push((
            program.display().to_string(),
            args.iter().map(|arg| arg.to_os_string()).collect(),
            merged,
        ));
        Ok(self.next.clone())
    }
}

#[test]
fn codesign_receives_the_unsplit_path_and_merged_output() {
    let mut adapter = SystemInspection::new(Scripted {
        next: b"Authority=Apple\n\n".to_vec(),
        ..Default::default()
    });
    assert_eq!(
        adapter.signing(Path::new("/a quoted path.app")),
        Ok(b"Authority=Apple".to_vec())
    );
    assert_eq!(
        adapter.runner.calls,
        vec![(
            "/usr/bin/codesign".into(),
            ["-dv", "--verbose=2", "/a quoted path.app"]
                .map(OsString::from)
                .to_vec(),
            true
        )]
    );
}

#[test]
fn plist_extraction_preserves_empty_values_and_exact_key_arguments() {
    let mut adapter = SystemInspection::new(Scripted {
        next: b"\n\n".to_vec(),
        ..Default::default()
    });
    assert_eq!(
        adapter.plist_value(Path::new("/x.plist"), "ProgramArguments.5"),
        Ok(Vec::new())
    );
    assert_eq!(
        adapter.runner.calls,
        vec![(
            "/usr/bin/plutil".into(),
            [
                "-extract",
                "ProgramArguments.5",
                "raw",
                "-o",
                "-",
                "/x.plist"
            ]
            .map(OsString::from)
            .to_vec(),
            false
        )]
    );
}

#[test]
fn quarantine_uses_the_selected_path_and_requires_nonempty_output() {
    let mut adapter = SystemInspection::new(Scripted {
        next: b"mark\n".to_vec(),
        ..Default::default()
    });
    assert!(adapter.quarantined(Path::new("/script path")));
    assert_eq!(
        adapter.runner.calls,
        vec![(
            "/usr/bin/xattr".into(),
            ["-p", "com.apple.quarantine", "/script path"]
                .map(OsString::from)
                .to_vec(),
            false
        )]
    );
    adapter.runner.next = b"\n".to_vec();
    assert!(!adapter.quarantined(Path::new("/script path")));
}

#[test]
fn only_regular_files_reach_file_and_its_mach_o_reading_selects_code() {
    let directory = std::env::temp_dir().join(format!("posture-file-kind-{}", std::process::id()));
    std::fs::create_dir(&directory).expect("private fixture directory");
    let path = directory.join("binary with spaces");
    std::fs::write(&path, b"inert fixture").expect("fixture contents");
    let mut adapter = SystemInspection::new(Scripted {
        next: b"fixture: MACH-O executable\n".to_vec(),
        ..Default::default()
    });
    assert!(adapter.is_mach_o(&path));
    assert_eq!(
        adapter.runner.calls,
        vec![(
            "/usr/bin/file".into(),
            vec![path.as_os_str().to_owned()],
            false
        )]
    );
    assert!(!adapter.is_mach_o(&directory));
    assert_eq!(
        adapter.runner.calls.len(),
        1,
        "directories must not spawn file"
    );
}

#[test]
fn command_substitution_removes_nul_before_signing_classification() {
    let mut adapter = SystemInspection::new(Scripted {
        next: b"Authority=Apple\nnot\0 signed".to_vec(),
        ..Default::default()
    });
    let bytes = adapter
        .signing(Path::new("/x.app"))
        .expect("scripted reading");
    assert_eq!(
        posture_domain::classify_signing(Some(&bytes)).exit_code(),
        10
    );
    assert_eq!(
        adapter.diagnostics(),
        b"posture: warning: ignored null byte in inspection output\n"
    );
}

#[test]
fn a_quarantine_attribute_containing_only_nul_is_empty() {
    let mut adapter = SystemInspection::new(Scripted {
        next: b"\0\n".to_vec(),
        ..Default::default()
    });
    assert!(!adapter.quarantined(Path::new("/x.app")));
    assert_eq!(
        adapter.diagnostics(),
        b"posture: warning: ignored null byte in inspection output\n"
    );
}
