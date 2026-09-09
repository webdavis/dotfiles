use super::*;
const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
fn fp() -> Option<AuditFingerprint> {
    AuditFingerprint::parse(A)
}
#[test]
fn audit_confirmation_pages_once_restarts_on_change_and_forgets_after_clean() {
    let first = judge_audit(false, "missing", fp(), &AuditMemory::default());
    assert_eq!(first.next.streak, 1);
    assert_eq!(first.problem, None);
    let second = judge_audit(false, "missing", fp(), &first.next);
    assert_eq!(second.next.streak, 2);
    assert!(
        second
            .problem
            .as_ref()
            .unwrap()
            .contains("manifest is missing or unreadable")
    );
    assert_eq!(second.next.paged, fp());
    assert_eq!(
        judge_audit(false, "missing", fp(), &second.next).problem,
        None
    );
    let changed = judge_audit(false, "budget", AuditFingerprint::parse(B), &second.next);
    assert_eq!(changed.next.streak, 1);
    assert_eq!(changed.next.paged, fp());
    assert_eq!(changed.problem, None);
    let clean = judge_audit(true, "", None, &second.next);
    assert_eq!(clean.next, AuditMemory::default());
    assert_eq!(clean.problem, None);
    let recur = judge_audit(false, "missing", fp(), &clean.next);
    assert_eq!(recur.next.streak, 1);
}
#[test]
fn invalid_fingerprints_page_every_tick_and_streaks_clamp_on_read_and_write() {
    for bad in [
        "",
        "a",
        "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaag",
    ] {
        assert_eq!(AuditFingerprint::parse(bad), None);
    }
    for (raw, expected) in [
        ("0", 0),
        ("98", 98),
        ("99", 99),
        ("100", 99),
        ("999", 99),
        ("1000", 0),
        ("-1", 0),
        ("x", 0),
    ] {
        let memory = AuditMemory::from_readings(A, raw, B);
        assert_eq!(memory.streak, expected);
        let result = judge_audit(false, "budget", fp(), &memory);
        assert_eq!(result.next.streak, (expected + 1).min(99));
    }
    for _ in 0..2 {
        let result = judge_audit(
            false,
            "budget",
            None,
            &AuditMemory::from_readings(A, "99", A),
        );
        assert!(result.problem.unwrap().contains("time budget"));
        assert_eq!(result.next, AuditMemory::default());
    }
    assert_eq!(
        AuditMemory::from_readings("bad", "4", "bad").fingerprint,
        None
    );
    assert_eq!(AuditMemory::from_readings("bad", "4", "bad").paged, None);
}
#[test]
fn audit_pages_render_only_fixed_kind_labels_and_count_columns_in_report_order() {
    let report =
        "owner /do-not-render\nmode /private/path content injected\ncontent /one\nunknown /other";
    let result = judge_audit(true, report, fp(), &AuditMemory::from_readings(A, "1", ""));
    assert_eq!(
        result.problem.as_deref(),
        Some(
            "4 divergence(s) from a known-good manifest (content changed, permissions changed, ownership changed); no file event reported this, which is what a hard-linked or relocated script, or a chmod through such an alias, looks like"
        )
    );
    let all = "unreadable /7\noversize /6\nirregular /5\nmissing /4\nowner /3\nmode /2\ncontent /1";
    let problem = judge_audit(true, all, None, &AuditMemory::default())
        .problem
        .unwrap();
    assert!(problem.contains("content changed, permissions changed, ownership changed, file missing, not a regular file, too large to hash, unreadable"));
    assert!(!problem.contains("/7"));
    for count in [1, 999_999] {
        let report = "x\n".repeat(count);
        assert!(
            judge_audit(true, &report, None, &AuditMemory::default())
                .problem
                .unwrap()
                .starts_with(&format!("{count} divergence(s)"))
        );
    }
    let too_many = "x\n".repeat(1_000_000);
    assert_eq!(
        judge_audit(true, &too_many, None, &AuditMemory::default())
            .problem
            .as_deref(),
        Some(
            "the periodic manifest audit could not verify the deployed files against their known-good manifests"
        )
    );
    assert_eq!(
        audit_fingerprint_input("mode /b\ncontent /a\nmode /b\n"),
        "content /a\nmode /b\nmode /b\n"
    );
}
#[test]
fn mixed_refusals_and_hostile_tokens_use_the_fixed_unknown_problem() {
    for report in [
        "content /private/path\nmalformed",
        "hostile $(secret)",
        "",
        "a",
    ] {
        assert_eq!(
            judge_audit(false, report, None, &AuditMemory::default())
                .problem
                .as_deref(),
            Some(
                "the periodic manifest audit could not verify the deployed files against their known-good manifests"
            )
        );
    }
    for (token, phrase) in [
        ("missing", "missing or unreadable"),
        ("unavailable", "not installed completely"),
        ("untrustworthy", "no longer root-owned"),
        ("malformed", "malformed entry"),
        ("overlong", "more files"),
        ("budget", "time budget"),
    ] {
        assert!(
            judge_audit(false, token, None, &AuditMemory::default())
                .problem
                .unwrap()
                .contains(phrase)
        );
    }
}
