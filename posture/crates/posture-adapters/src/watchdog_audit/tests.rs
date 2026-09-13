use super::*;
use sha2::{Digest, Sha256};
use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt, symlink},
    sync::atomic::{AtomicU64, Ordering},
};
fn subject() -> WatchdogAudit {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "posture-watchdog-audit-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&dir).unwrap();
    let pns = dir.join("pns");
    fs::write(&pns, b"authorized").unwrap();
    fs::set_permissions(&pns, fs::Permissions::from_mode(0o755)).unwrap();
    let tuple = format!(
        "{} 0755 {} {}\n",
        Sha256::digest(b"authorized")
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>(),
        fs::metadata(&pns).unwrap().uid(),
        pns.display()
    );
    let pipeline = dir.join("pipeline");
    let managed_bin = dir.join("bin");
    fs::write(&pipeline, &tuple).unwrap();
    fs::write(&managed_bin, &tuple).unwrap();
    WatchdogAudit {
        pipeline,
        managed_bin,
        pns,
        authority: [ManifestAuthority::ExplicitOverride; 2],
        bounds: AuditBounds::from_values("500", "8388608", "60"),
    }
}
#[test]
fn scheduled_audit_checks_every_bound_column_without_rendering_paths() {
    let mut audit = subject();
    assert!(audit.pipeline().completed);
    assert!(audit.pipeline().report.is_empty());
    fs::write(&audit.pns, b"swapped").unwrap();
    fs::set_permissions(&audit.pns, fs::Permissions::from_mode(0o644)).unwrap();
    let report = audit.pipeline();
    assert!(report.completed);
    assert_eq!(report.report.lines().count(), 4);
    assert!(report.report.contains("content "));
    assert!(report.report.contains("mode "));
    assert!(report.fingerprint.is_some());
    let problem = audit.pns_problem().unwrap();
    assert!(problem.contains("pns"));
    assert!(!problem.contains(&audit.pns.display().to_string()));
}
#[test]
fn independent_pns_check_requires_its_exact_authorized_tuple() {
    let mut audit = subject();
    assert_eq!(audit.pns_problem(), None);
    for line in ["", "unbuilt 0755 501 /elsewhere\n", "bad tuple\n"] {
        fs::write(&audit.pipeline, line).unwrap();
        assert!(audit.pns_problem().is_some());
    }
    let mut audit = subject();
    let row = fs::read_to_string(&audit.pipeline).unwrap();
    fs::write(&audit.pipeline, format!("{row}{row}")).unwrap();
    assert!(audit.pns_problem().is_some());
}
#[test]
fn missing_and_untrusted_manifests_cannot_report_an_all_clear() {
    let mut audit = subject();
    audit.pipeline = audit.pipeline.with_extension("absent");
    let report = audit.pipeline();
    assert!(!report.completed);
    assert_eq!(report.report, "missing\n");
    let mut audit = subject();
    audit.authority = [ManifestAuthority::Protected; 2];
    let report = audit.pipeline();
    assert!(!report.completed);
    assert_eq!(report.report, "untrustworthy\n");
    assert!(audit.pns_problem().is_some());
}
#[test]
fn limits_and_late_refusal_retain_prior_findings() {
    let mut audit = subject();
    fs::write(&audit.pns, b"tampered").unwrap();
    fs::write(&audit.managed_bin, b"malformed\n").unwrap();
    let report = audit.pipeline();
    assert!(!report.completed);
    assert!(report.report.starts_with("content "));
    assert!(report.report.ends_with("malformed\n"));
    let mut audit = subject();
    audit.bounds.seconds = 0;
    assert_eq!(audit.pipeline().report, "budget\n");
    let mut audit = subject();
    audit.bounds.entries = 1;
    let tuple = fs::read_to_string(&audit.pipeline).unwrap();
    fs::write(&audit.pipeline, format!("{tuple}{tuple}")).unwrap();
    assert!(audit.pipeline().report.ends_with("overlong\n"));
}
#[test]
fn symlinks_and_unbuilt_regular_files_are_never_trusted_as_matching_bytes() {
    let mut audit = subject();
    let link = audit.pns.with_extension("link");
    symlink(&audit.pns, &link).unwrap();
    let row = fs::read_to_string(&audit.pipeline)
        .unwrap()
        .replace(audit.pns.to_str().unwrap(), link.to_str().unwrap());
    fs::write(&audit.pipeline, row).unwrap();
    assert!(audit.pipeline().report.starts_with("irregular "));
    let mut audit = subject();
    let row = format!(
        "unbuilt 0755 {} {}\n",
        fs::metadata(&audit.pns).unwrap().uid(),
        audit.pns.display()
    );
    fs::write(&audit.pipeline, row).unwrap();
    assert!(audit.pipeline().report.starts_with("content "));
    assert!(audit.pns_problem().is_some());
}

#[test]
fn oversize_files_still_report_their_attribute_drift() {
    let mut audit = subject();
    audit.bounds.bytes = 1;
    fs::set_permissions(&audit.pns, fs::Permissions::from_mode(0o644)).unwrap();
    let report = audit.pipeline();
    assert!(report.completed);
    assert!(report.report.contains("oversize "));
    assert!(report.report.contains("mode "));
    assert!(audit.pns_problem().is_some());
}

#[test]
fn unreadable_content_retains_observed_mode_and_owner_drift() {
    let mut audit = subject();
    let owner = fs::metadata(&audit.pns).unwrap().uid();
    let row = fs::read_to_string(&audit.pipeline).unwrap();
    fs::write(
        &audit.pipeline,
        row.replace(&format!("0755 {owner} "), &format!("0755 {} ", owner + 1)),
    )
    .unwrap();
    fs::set_permissions(&audit.pns, fs::Permissions::from_mode(0o000)).unwrap();
    assert!(fs::File::open(&audit.pns).is_err());
    assert_eq!(fs::symlink_metadata(&audit.pns).unwrap().mode() & 0o7777, 0);
    let report = audit.pipeline();
    fs::set_permissions(&audit.pns, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(report.completed);
    for kind in ["unreadable ", "mode ", "owner "] {
        assert!(
            report.report.contains(kind),
            "lost {kind}: {}",
            report.report
        );
    }
}
