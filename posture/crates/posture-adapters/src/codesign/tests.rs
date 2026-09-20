use super::*;
use std::ffi::{OsStr, OsString};

#[derive(Default)]
struct Scripted {
    next: Vec<u8>,
    calls: Vec<(String, Vec<OsString>, bool)>,
}
impl CommandRunner for Scripted {
    fn run_completed(
        &mut self,
        program: &Path,
        args: &[&OsStr],
        io: CommandIo,
    ) -> Result<crate::CommandOutput, InspectionFailure> {
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
        Ok(crate::CommandOutput {
            bytes: self.next.clone(),
            exit: 0,
        })
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
fn plist_extraction_preserves_empty_values_and_reads_no_child_process() {
    let sandbox = crate::test_sandbox::Sandbox::new("plist-value");
    let path = sandbox.join("job.plist");
    std::fs::write(
        &path,
        br#"<plist version="1.0"><dict><key>Program</key><string>/bin/sh</string><key>ProgramArguments</key><array><string>/bin/sh</string><string/></array></dict></plist>"#,
    )
    .expect("fixture contents");
    let mut adapter = SystemInspection::new(Scripted::default());
    assert_eq!(
        adapter.plist_value(&path, "ProgramArguments.1"),
        Ok(Vec::new())
    );
    assert_eq!(
        adapter.plist_value(&path, "Program"),
        Ok(b"/bin/sh".to_vec())
    );
    assert_eq!(
        adapter.plist_value(&path, "ProgramArguments.5"),
        Err(InspectionFailure::Failed)
    );
    assert_eq!(
        adapter.plist_value(&sandbox.join("absent.plist"), "Program"),
        Err(InspectionFailure::Failed)
    );
    assert!(
        adapter.runner.calls.is_empty(),
        "no child process reads a plist"
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
    let sandbox = crate::test_sandbox::Sandbox::new("file-kind");
    let directory = sandbox.path();
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
    assert!(!adapter.is_mach_o(directory));
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
