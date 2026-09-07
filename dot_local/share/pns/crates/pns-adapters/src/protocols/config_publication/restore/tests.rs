use super::*;
use crate::state_fixtures::scratch;

#[test]
fn a_backup_security_failure_restores_the_old_config_and_reports_the_failure() {
    let state = scratch("setup-security");
    let path = state.join("config.toml");
    let backup = state.join("backup");
    std::fs::write(&backup, "old config").unwrap();
    let refusal = secure_backup(&path, &backup, |_| {
        Err(std::io::ErrorKind::PermissionDenied.into())
    })
    .unwrap_err();
    assert!(refusal.contains("could not be secured"), "{refusal}");
    assert!(refusal.contains("restored"), "{refusal}");
    assert_eq!(std::fs::read(path).unwrap(), b"old config");
    assert_eq!(std::fs::read(backup).unwrap(), b"old config");
}

#[test]
fn restoration_preserves_a_later_config_and_the_old_backup() {
    let state = scratch("setup-new-arrival");
    let path = state.join("config.toml");
    let backup = state.join("backup");
    std::fs::write(&backup, "old config").unwrap();
    std::fs::write(&path, "later config").unwrap();
    let report = restore_after_failure(&path, Some(&backup));
    assert!(report.contains("left untouched"), "{report}");
    assert!(!report.contains("was restored"), "{report}");
    assert_eq!(std::fs::read(path).unwrap(), b"later config");
    assert_eq!(std::fs::read(backup).unwrap(), b"old config");
}

#[test]
fn restoration_failure_keeps_the_backup_and_names_the_unrestored_state() {
    let state = scratch("setup-no-directory");
    let path = state.join("absent/config.toml");
    let backup = state.join("backup");
    std::fs::write(&backup, "old config").unwrap();
    let report = restore_after_failure(&path, Some(&backup));
    assert!(report.contains("could not be restored"), "{report}");
    assert!(report.contains(&backup.display().to_string()), "{report}");
    assert!(!path.exists());
    assert_eq!(std::fs::read(backup).unwrap(), b"old config");
}
