use super::*;

const HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const OTHER: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const LINE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 0644 0 /private/captured-path";

fn regular<'a>(size: u64, mode: &'a str, uid: &'a str, digest: &'a str) -> AuditFile<'a> {
    AuditFile::Regular {
        size: Some(size),
        mode: Some(mode),
        uid: Some(uid),
        digest: Some(digest),
    }
}

fn row<'a>(line: &'a str, file: AuditFile<'a>) -> AuditRow<'a> {
    AuditRow {
        line,
        checked_at: 100,
        file,
    }
}

fn scan<'a>(first: &'a [AuditRow<'a>], second: &'a [AuditRow<'a>]) -> AuditReport<'a> {
    audit_scan(
        Some([AuditManifest::Rows(first), AuditManifest::Rows(second)]),
        AuditBounds::from_values("500", "8388608", "60"),
        100,
    )
}

#[test]
fn an_audit_keeps_earlier_findings_when_either_manifest_refuses() {
    let changed = row(LINE, regular(1, "0644", "0", OTHER));
    let malformed = row("malformed", AuditFile::Missing);
    let later = row(LINE, regular(1, "0644", "0", HASH));
    for report in [
        scan(&[changed, malformed], &[later]),
        scan(&[changed], &[malformed, later]),
    ] {
        assert_eq!(report.refusal, Some(AuditRefusal::Malformed));
        assert_eq!(report.text(), "content /private/captured-path\nmalformed\n");
    }
}

#[test]
fn each_drift_column_is_reported_in_manifest_order() {
    let all = row(LINE, regular(1, "0600", "501", OTHER));
    let missing_line = format!("{HASH} 0644 0 /path with spaces");
    let missing = row(&missing_line, AuditFile::Missing);
    let first = [all];
    let second = [missing];
    let report = scan(&first, &second);
    assert_eq!(report.refusal, None);
    assert_eq!(
        report.text(),
        "content /private/captured-path\nmode /private/captured-path\nowner /private/captured-path\nmissing /path with spaces\n"
    );
}

#[test]
fn file_kind_and_read_failures_keep_distinct_audit_findings() {
    let clean = row(LINE, regular(1, "0644", "0", HASH));
    for (file, kinds) in [
        (AuditFile::Missing, vec![AuditKind::Missing]),
        (AuditFile::Symlink, vec![AuditKind::Irregular]),
        (AuditFile::Irregular, vec![AuditKind::Irregular]),
        (
            regular(8_388_609, "0600", "501", "unread"),
            vec![AuditKind::Oversize, AuditKind::Mode, AuditKind::Owner],
        ),
        (
            regular(1, "0600", "0", "invalid"),
            vec![AuditKind::Unreadable, AuditKind::Mode],
        ),
        (regular(1, "644", "501", OTHER), vec![AuditKind::Unreadable]),
        (
            regular(1, "0644", "not numeric", OTHER),
            vec![AuditKind::Unreadable],
        ),
        (
            AuditFile::Regular {
                size: None,
                mode: Some("0600"),
                uid: Some("501"),
                digest: Some(OTHER),
            },
            vec![AuditKind::Unreadable],
        ),
        (regular(8_388_608, "0644", "0", HASH), vec![]),
    ] {
        let first = [row(LINE, file)];
        let second = [clean];
        let report = scan(&first, &second);
        assert_eq!(
            report.findings.iter().map(|f| f.kind).collect::<Vec<_>>(),
            kinds
        );
    }
}

#[test]
fn audit_bounds_refuse_at_entry_and_shared_deadline_edges() {
    let one = row(LINE, regular(1, "0644", "0", OTHER));
    let single = [one];
    let bounds = AuditBounds::from_values("1", "8388608", "1");
    let accepted = audit_scan(
        Some([AuditManifest::Rows(&single), AuditManifest::Rows(&single)]),
        bounds,
        100,
    );
    assert_eq!(accepted.findings.len(), 2);
    assert_eq!(accepted.refusal, None);
    let too_many = [one, one];
    let overlong = audit_scan(
        Some([AuditManifest::Rows(&too_many), AuditManifest::Missing]),
        bounds,
        100,
    );
    assert_eq!(overlong.findings.len(), 1);
    assert_eq!(overlong.refusal, Some(AuditRefusal::Overlong));
    let exhausted = [AuditRow {
        checked_at: 101,
        ..one
    }];
    let budget = audit_scan(
        Some([
            AuditManifest::Rows(&single),
            AuditManifest::Rows(&exhausted),
        ]),
        bounds,
        100,
    );
    assert_eq!(budget.findings.len(), 1);
    assert_eq!(budget.refusal, Some(AuditRefusal::Budget));
    let immediate = audit_scan(
        Some([AuditManifest::Rows(&single), AuditManifest::Rows(&single)]),
        AuditBounds {
            seconds: 0,
            ..bounds
        },
        100,
    );
    assert!(immediate.findings.is_empty());
    assert_eq!(immediate.refusal, Some(AuditRefusal::Budget));
}

#[test]
fn audit_limits_accept_both_edges_and_refuse_noncanonical_or_outside_values() {
    for (entries, bytes, seconds, expected) in [
        (
            "1",
            "1",
            "0",
            AuditBounds {
                entries: 1,
                bytes: 1,
                seconds: 0,
            },
        ),
        (
            "100000",
            "1073741824",
            "300",
            AuditBounds {
                entries: 100_000,
                bytes: 1_073_741_824,
                seconds: 300,
            },
        ),
        (
            "0",
            "0",
            "-1",
            AuditBounds {
                entries: 500,
                bytes: 8_388_608,
                seconds: 60,
            },
        ),
        (
            "100001",
            "1073741825",
            "301",
            AuditBounds {
                entries: 500,
                bytes: 8_388_608,
                seconds: 60,
            },
        ),
        (
            "01",
            "00",
            "060",
            AuditBounds {
                entries: 500,
                bytes: 8_388_608,
                seconds: 60,
            },
        ),
        (
            "99999999999",
            " 3",
            "$(bad)",
            AuditBounds {
                entries: 500,
                bytes: 8_388_608,
                seconds: 60,
            },
        ),
    ] {
        assert_eq!(AuditBounds::from_values(entries, bytes, seconds), expected);
    }
}

#[test]
fn an_unbuilt_regular_artifact_is_content_drift_before_attribute_reads() {
    let unbuilt = [row(
        "unbuilt 0644 0 /private/captured-path",
        AuditFile::Regular {
            size: None,
            mode: None,
            uid: None,
            digest: None,
        },
    )];
    let malformed = [row("malformed", AuditFile::Missing)];
    let report = scan(&unbuilt, &malformed);
    assert_eq!(report.text(), "content /private/captured-path\nmalformed\n");
}

#[test]
fn missing_untrusted_and_malformed_manifests_never_become_an_all_clear() {
    let bounds = AuditBounds::from_values("500", "8388608", "60");
    assert_eq!(
        audit_scan(None, bounds, 100).refusal,
        Some(AuditRefusal::Unavailable)
    );
    for (manifest, expected) in [
        (AuditManifest::Missing, AuditRefusal::Missing),
        (AuditManifest::Rows(&[]), AuditRefusal::Missing),
        (AuditManifest::Untrustworthy, AuditRefusal::Untrustworthy),
    ] {
        assert_eq!(
            audit_scan(Some([manifest, AuditManifest::Missing]), bounds, 100).refusal,
            Some(expected)
        );
    }
    for line in [
        "",
        "# comment",
        "hash /old-content-only",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 0644 0 relative",
    ] {
        assert_eq!(
            scan(&[row(line, AuditFile::Missing)], &[]).refusal,
            Some(AuditRefusal::Malformed)
        );
    }
}
