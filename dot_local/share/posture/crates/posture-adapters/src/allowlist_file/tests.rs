use super::*;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

fn fixture(bytes: &[u8]) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "posture-source-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::write(&path, bytes).unwrap();
    path
}
#[test]
fn listing_preserves_entry_bytes_and_only_skips_empty_or_leading_comment_lines() {
    let path = fixture(b"# ignored\n\n  # retained\n{bad\n\xff\r\nlast");
    assert_eq!(
        listed_bytes(&path),
        Ok(b"  # retained\n{bad\n\xff\r\nlast\n".to_vec())
    );
}
#[test]
fn missing_and_empty_deployed_lists_both_print_nothing() {
    assert_eq!(listed_bytes(&fixture(b"")), Ok(Vec::new()));
    assert_eq!(
        listed_bytes(&fixture(b"").with_extension("absent")),
        Ok(Vec::new())
    );
}
#[test]
fn deny_membership_is_a_raw_compact_substring_even_inside_a_comment() {
    assert!(!contains_label_text(
        &fixture(b"{\"label\": \"my.alpha\"}\n"),
        "my.alpha"
    ));
    assert!(contains_label_text(
        &fixture(b"# {\"label\":\"my.alpha\"}\n{bad\n"),
        "my.alpha"
    ));
    assert!(!contains_label_text(&fixture(b"{bad\n"), "my.alpha"));
}
#[test]
fn listing_matches_bash_read_nul_discard_before_comment_and_blank_detection() {
    assert_eq!(
        listed_bytes(&fixture(b"\0# ignored\nA\0B\n\0\n")),
        Ok(b"AB\n".to_vec())
    );
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
    fn run(
        &mut self,
        program: &Path,
        args: &[&OsStr],
        io: CommandIo,
    ) -> Result<Vec<u8>, InspectionFailure> {
        assert_eq!(program, Path::new("/fixture/chezmoi"));
        assert_eq!(args, [OsStr::new("source-path"), self.target.as_os_str()]);
        assert_eq!(io, CommandIo::CaptureStdout);
        self.answer.clone()
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
    let path = fixture(b"# retained\n\n{\"label\":\"my.alpha\",\"unused\":NaN}\n{bad");
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
