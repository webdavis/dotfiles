use super::*;
use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::OsStrExt;

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

/// Marks a real file the way a download is marked, so the reading under test
/// meets the attribute the system would have set.
fn set_quarantine(path: &Path, value: &[u8]) {
    let path = std::ffi::CString::new(path.as_os_str().as_bytes()).expect("a fixture path");
    let status = unsafe {
        libc::setxattr(
            path.as_ptr(),
            QUARANTINE.as_ptr().cast::<libc::c_char>(),
            value.as_ptr().cast::<libc::c_void>(),
            value.len(),
            0,
            0,
        )
    };
    assert_eq!(status, 0, "the fixture attribute must be set");
}

#[test]
fn quarantine_reads_the_attribute_of_the_selected_path_and_spawns_nothing() {
    let sandbox = crate::test_sandbox::Sandbox::new("quarantine");
    let marked = sandbox.join("marked path");
    let plain = sandbox.join("plain path");
    std::fs::write(&marked, b"inert fixture").expect("fixture contents");
    std::fs::write(&plain, b"inert fixture").expect("fixture contents");
    set_quarantine(&marked, b"0083;68c0f0c0;Safari;");
    let mut adapter = SystemInspection::new(Scripted::default());
    assert!(adapter.quarantined(&marked));
    assert!(!adapter.quarantined(&plain));
    assert!(!adapter.quarantined(&sandbox.join("absent path")));
    assert!(
        adapter.runner.calls.is_empty(),
        "no child process reads xattr"
    );
}

#[test]
fn only_regular_files_are_read_and_a_mach_o_magic_selects_code() {
    let sandbox = crate::test_sandbox::Sandbox::new("file-kind");
    let directory = sandbox.path();
    let mut adapter = SystemInspection::new(Scripted::default());
    for magic in MACH_O_MAGICS {
        let path = directory.join(format!("binary with spaces {magic:08x}"));
        let mut contents = magic.to_be_bytes().to_vec();
        contents.extend_from_slice(b"the rest of an object");
        std::fs::write(&path, &contents).expect("fixture contents");
        assert!(adapter.is_mach_o(&path), "{magic:08x}");
    }
    let text = directory.join("notes.txt");
    std::fs::write(&text, b"inert fixture").expect("fixture contents");
    assert!(!adapter.is_mach_o(&text));
    let short = directory.join("three bytes");
    std::fs::write(&short, b"\xfe\xed\xfa").expect("fixture contents");
    assert!(!adapter.is_mach_o(&short));
    assert!(!adapter.is_mach_o(directory));
    let literal = directory.join("literal fat64 magic");
    std::fs::write(&literal, 0xfeed_facf_u32.to_be_bytes()).expect("fixture contents");
    assert!(adapter.is_mach_o(&literal));
    assert!(
        adapter.is_mach_o(Path::new("/usr/bin/true")),
        "a real Mach-O binary must classify as code"
    );
    assert!(
        adapter.runner.calls.is_empty(),
        "no child process reads a magic"
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
    let sandbox = crate::test_sandbox::Sandbox::new("quarantine-nul");
    let path = sandbox.join("marked");
    std::fs::write(&path, b"inert fixture").expect("fixture contents");
    set_quarantine(&path, b"\0");
    let mut adapter = SystemInspection::new(Scripted::default());
    assert!(!adapter.quarantined(&path));
    assert_eq!(
        adapter.diagnostics(),
        b"posture: warning: ignored null byte in inspection output\n"
    );
}
