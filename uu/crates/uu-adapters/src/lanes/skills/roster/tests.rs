use super::*;
use crate::lanes::skills::tests::{directory, roster, write_roster};
#[test]
fn a_lock_that_is_missing_unparseable_or_schema_broken_refuses_the_run_by_name() {
    let p = directory().join("roster.json");
    let mut inputs = vec![None, Some("{".into()), Some(format!("{} {{}}", roster()))];
    for (key, bad) in [
        ("version", serde_json::json!(1)),
        ("npxTracked", serde_json::json!(false)),
        (
            "clawhubTracked",
            serde_json::json!({"gamma":{"slug":"../escape","registry":""}}),
        ),
        ("tiers", serde_json::json!([])),
        ("claudeDelivery", serde_json::json!({"alpha":"yes"})),
        ("hermesProfiles", serde_json::json!({"alpha":["../escape"]})),
    ] {
        let mut v = roster();
        v[key] = bad;
        inputs.push(Some(v.to_string()));
    }
    for text in inputs {
        if let Some(text) = text {
            std::fs::write(&p, text).unwrap();
        }
        let error = SkillsRoster::read(&p).unwrap_err();
        assert!(error.contains("roster.json"), "{error}");
    }
}
#[test]
fn a_roster_tracking_zero_skills_is_refused_as_corruption_not_intent() {
    let p = directory().join("roster.json");
    write_roster(&p, &serde_json::json!({"version":2}));
    assert!(SkillsRoster::read(&p).unwrap_err().contains("zero skills"));
}
#[test]
fn the_tracked_set_is_the_union_of_the_npx_and_clawhub_tables() {
    let p = directory().join("roster.json");
    write_roster(&p, &roster());
    let got = SkillsRoster::read(&p).unwrap();
    assert_eq!(
        got.tracked_names(),
        BTreeSet::from(["alpha".into(), "beta".into(), "gamma".into()])
    );
    assert_eq!(got.npx["alpha"], "owner/repo");
    assert_eq!(got.hermes_profiles["alpha"], vec!["default"]);
}
#[test]
fn a_roster_that_changed_mid_run_refuses_the_publish() {
    let p = directory().join("roster.json");
    write_roster(&p, &roster());
    let captured = SkillsRoster::read(&p).unwrap();
    assert!(captured.unchanged().is_ok());
    let mut changed = roster();
    changed["npxTracked"]["alpha"]["repo"] = "different/repo".into();
    write_roster(&p, &changed);
    assert!(captured.unchanged().unwrap_err().contains("changed"));
    assert_eq!(captured.npx["alpha"], "owner/repo");
}
#[test]
fn a_skill_in_both_hermes_registry_and_a_non_empty_hermes_profiles_row_is_refused() {
    let p = directory().join("roster.json");
    let mut v = roster();
    v["hermesRegistry"] = serde_json::json!({"alpha":{"profiles":["default"],"source":"clawhub","identifier":"clawhub/alpha","lockKey":"alpha"}});
    write_roster(&p, &v);
    let error = SkillsRoster::read(&p).unwrap_err();
    assert!(
        error.contains("alpha") && error.contains("hermesRegistry"),
        "{error}"
    );
    v["hermesProfiles"]["alpha"] = serde_json::json!([]);
    write_roster(&p, &v);
    assert!(SkillsRoster::read(&p).is_ok());
}
