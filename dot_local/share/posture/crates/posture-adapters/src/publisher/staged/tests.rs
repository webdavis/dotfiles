use super::*;
use posture_domain::{AllowlistEntry, LaunchdIdentity};
use std::os::unix::fs::PermissionsExt;
#[test]
fn encoding_preserves_raw_bytes_and_emits_the_fixed_four_field_tuple() {
    let entry = AllowlistEntry {
        identity: LaunchdIdentity {
            label: "my.agent",
            path: "~/a \"quoted\".plist",
            program: "~/run\nwith argument",
        },
        sha256: "abcdef",
    };
    assert_eq!(encode(&[CuratedLine::Preserved(b"# raw \xff"),CuratedLine::Added(entry)]).unwrap(),b"# raw \xff\n{\"label\":\"my.agent\",\"path\":\"~/a \\\"quoted\\\".plist\",\"program\":\"~/run\\nwith argument\",\"sha256\":\"abcdef\"}\n");
}
#[test]
fn staged_source_has_private_permissions_and_exact_bytes_before_publication() {
    let staged = Staged::create(b"private source\n").unwrap();
    assert_eq!(fs::read(&staged.0).unwrap(), b"private source\n");
    assert_eq!(
        fs::metadata(&staged.0).unwrap().permissions().mode() & 0o777,
        0o600
    );
}
