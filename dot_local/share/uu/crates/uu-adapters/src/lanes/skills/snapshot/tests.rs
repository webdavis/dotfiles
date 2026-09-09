use super::*;
use std::fs;
fn fixture() -> std::path::PathBuf {
    let agents = super::super::tests::directory();
    fs::create_dir_all(agents.join(".skills-current")).unwrap();
    fs::create_dir_all(agents.join("skills/gamma/.clawhub")).unwrap();
    fs::write(
        agents.join("skills/gamma/.clawhub/origin.json"),
        r#"{"installedVersion":"1.0"}"#,
    )
    .unwrap();
    agents
}
fn said(snapshot: &Snapshot) -> Vec<String> {
    let mut report = LaneReport::new("skills");
    snapshot.report(snapshot, &mut report);
    report.lines
}
#[test]
fn an_absent_npx_lock_is_empty_but_an_invalid_reading_never_claims_a_quiet_week() {
    let agents = fixture();
    let empty = said(&Snapshot::read(&agents));
    assert!(empty[0].contains("0 of 0"), "{empty:?}");
    assert!(empty[1].contains("0 of 1"), "{empty:?}");
    let lock = agents.join(".skills-current/.skill-lock.json");
    for invalid in ["broken", "", r#"{"skills":{}}{"skills":{}}"#] {
        fs::write(&lock, invalid).unwrap();
        let lines = said(&Snapshot::read(&agents));
        assert!(lines[0].contains("NOT COMPARED"), "{lines:?}");
        assert!(!lines[0].contains("0 of 0"), "{lines:?}");
        assert!(lines[1].contains("0 of 1"), "{lines:?}");
    }
    fs::write(&lock, r#"{"skills":{"alpha":{"skillFolderHash":"hash"}}}"#).unwrap();
    assert_eq!(
        Snapshot::read(&agents).npx.unwrap(),
        vec![("alpha".into(), "hash".into())]
    );
}
#[test]
fn an_invalid_clawhub_origin_marks_only_its_own_reading_uncompared() {
    let agents = fixture();
    fs::write(
        agents.join(".skills-current/.skill-lock.json"),
        r#"{"skills":{"alpha":{"skillFolderHash":"hash"}}}"#,
    )
    .unwrap();
    let origin = agents.join("skills/gamma/.clawhub/origin.json");
    for invalid in ["broken", "", r#"{"installedVersion":"1"}{}"#] {
        fs::write(&origin, invalid).unwrap();
        let lines = said(&Snapshot::read(&agents));
        assert!(lines[0].contains("0 of 1"), "{lines:?}");
        assert!(lines[1].contains("NOT COMPARED"), "{lines:?}");
        assert!(!lines[1].contains("tracked entries changed"), "{lines:?}");
    }
    fs::write(&origin, r#"{"installedVersion":"2.0"}"#).unwrap();
    assert_eq!(
        Snapshot::read(&agents).clawhub.unwrap(),
        vec![("gamma".into(), "2.0".into())]
    );
}
#[test]
fn a_publisher_version_cannot_forge_a_snapshot_row_or_report_column() {
    let agents = fixture();
    let before = Snapshot::read(&agents);
    fs::write(
        agents.join("skills/gamma/.clawhub/origin.json"),
        serde_json::json!({"installedVersion":"2.0\nforged\t9.9`"}).to_string(),
    )
    .unwrap();
    let after = Snapshot::read(&agents);
    assert_eq!(after.clawhub.as_ref().unwrap().len(), 1);
    let mut report = LaneReport::new("skills");
    before.report(&after, &mut report);
    let line = &report.lines[1];
    assert!(line.contains("1 of 1 tracked entries changed"), "{line}");
    assert!(line.contains("`gamma` `1.0` -> `2.0forged9.9`"), "{line}");
    assert!(!line.contains(['\n', '\t']), "{line}");
}
