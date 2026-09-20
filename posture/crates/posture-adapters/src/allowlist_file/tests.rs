use super::*;
use crate::test_sandbox::Sandbox;
use std::path::PathBuf;

/// A list file in a directory that removes itself. Hold the sandbox for as
/// long as the path is used; dropping it early takes the file with it.
fn fixture(bytes: &[u8]) -> (Sandbox, PathBuf) {
    let sandbox = Sandbox::new("source");
    let path = sandbox.join("allowlist");
    fs::write(&path, bytes).unwrap();
    (sandbox, path)
}
#[test]
fn listing_preserves_entry_bytes_and_only_skips_empty_or_leading_comment_lines() {
    let (_sandbox, path) = fixture(b"# ignored\n\n  # retained\n{bad\n\xff\r\nlast");
    assert_eq!(
        listed_bytes(&path),
        Ok(b"  # retained\n{bad\n\xff\r\nlast\n".to_vec())
    );
}
#[test]
fn missing_and_empty_deployed_lists_both_print_nothing() {
    let (_sandbox, empty) = fixture(b"");
    assert_eq!(listed_bytes(&empty), Ok(Vec::new()));
    assert_eq!(
        listed_bytes(&empty.with_extension("absent")),
        Ok(Vec::new())
    );
}
#[test]
fn deny_membership_is_a_raw_compact_substring_even_inside_a_comment() {
    let (_object_sandbox, object) = fixture(b"{\"label\": \"my.alpha\"}\n");
    assert!(!contains_label_text(&object, "my.alpha"));
    let (_commented_sandbox, commented) = fixture(b"# {\"label\":\"my.alpha\"}\n{bad\n");
    assert!(contains_label_text(&commented, "my.alpha"));
    let (_torn_sandbox, torn) = fixture(b"{bad\n");
    assert!(!contains_label_text(&torn, "my.alpha"));
}
#[test]
fn listing_matches_bash_read_nul_discard_before_comment_and_blank_detection() {
    let (_sandbox, path) = fixture(b"\0# ignored\nA\0B\n\0\n");
    assert_eq!(listed_bytes(&path), Ok(b"AB\n".to_vec()));
}

use crate::{CommandIo, CommandRunner};
use posture_application::{InspectionFailure, SourceAllowlist, SourceLine};
use std::ffi::OsStr;
use std::time::Duration;
struct Resolve {
    answer: Result<Vec<u8>, InspectionFailure>,
    target: PathBuf,
}
impl CommandRunner for Resolve {
    fn run_completed(
        &mut self,
        program: &Path,
        args: &[&OsStr],
        io: CommandIo,
    ) -> Result<crate::CommandOutput, InspectionFailure> {
        assert_eq!(program, Path::new("/fixture/chezmoi"));
        assert_eq!(args, [OsStr::new("source-path"), self.target.as_os_str()]);
        assert_eq!(io, CommandIo::CaptureStdout);
        self.answer
            .clone()
            .map(|bytes| crate::CommandOutput { bytes, exit: 0 })
    }
}
fn adapter(target: PathBuf) -> AllowlistFile {
    AllowlistFile::new(
        target,
        "/fixture/chezmoi".into(),
        Duration::from_millis(100),
    )
}
#[test]
fn source_resolution_preserves_argument_boundaries_and_command_substitution_bytes() {
    let target = PathBuf::from("/deployed with spaces");
    let f = adapter(target.clone());
    let mut runner = Resolve {
        target,
        answer: Ok(b"/source\0 with spaces\n\n".to_vec()),
    };
    assert_eq!(
        f.resolve_with(&mut runner),
        Ok(PathBuf::from("/source with spaces"))
    );
    for answer in [
        Ok(vec![]),
        Err(InspectionFailure::Failed),
        Err(InspectionFailure::TimedOut),
    ] {
        runner.answer = answer;
        assert_eq!(f.resolve_with(&mut runner), Err(SourceRefusal::Resolve));
    }
}
#[test]
fn source_read_preserves_blank_lines_and_torn_final_entry_with_typed_projection() {
    let (_sandbox, path) = fixture(b"# retained\n\n{\"label\":\"my.alpha\",\"unused\":NaN}\n{bad");
    let f = adapter("/deployed".into());
    assert_eq!(
        f.read(&path),
        Ok(vec![
            SourceLine::Preserved(b"# retained".to_vec()),
            SourceLine::Preserved(vec![]),
            SourceLine::Object {
                label: Some("my.alpha".into()),
                raw: b"{\"label\":\"my.alpha\",\"unused\":NaN}".to_vec()
            },
            SourceLine::Invalid
        ])
    );
    assert_eq!(f.read(&path.with_extension("absent")), Ok(vec![]));
}
