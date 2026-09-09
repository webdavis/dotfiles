use super::*;
use crate::lanes::skills::tests::{directory, roster, write_roster};
use crate::lanes::stubs::ScriptedRunner;
fn setup() -> (SkillsRoster, SkillsConfig) {
    let root = directory();
    let path = root.join("roster");
    let mut value = roster();
    value["hermesRegistry"] = serde_json::json!({
        "first": {"source":"hub","identifier":"display-first","lockKey":"@owner/first","profiles":["default","writer"]},
        "second": {"source":"hub","identifier":"display-second","lockKey":"@owner/second","profiles":["writer"]}
    });
    write_roster(&path, &value);
    (
        SkillsRoster::read(&path).unwrap(),
        SkillsConfig {
            lock: path.to_string_lossy().into(),
            agents: root.join(".agents").to_string_lossy().into(),
            claude_skills: root.join(".claude/skills").to_string_lossy().into(),
            hermes: root.join(".hermes").to_string_lossy().into(),
            npx: "/npx".into(),
            skills_cli_version: "1.5.22".into(),
            clawhub: "/clawhub".into(),
            hermes_cli: "/fixture/hermes".into(),
            cua_driver: "/cua-driver".into(),
            routing: "/routing".into(),
        },
    )
}
#[test]
fn every_registry_entry_is_updated_in_each_of_its_profiles_by_lock_key() {
    let (roster, config) = setup();
    let runner = ScriptedRunner::new(&[&[
        "/fixture/hermes",
        "-p",
        "default",
        "skills",
        "update",
        "@owner/first",
    ]])
    .answering("updated detail");
    let mut report = LaneReport::new("skills");
    roster.update_hermes_registry(&config, &runner, &mut report);
    assert_eq!(
        runner.calls(),
        vec![
            vec![
                "/fixture/hermes",
                "-p",
                "default",
                "skills",
                "update",
                "@owner/first"
            ],
            vec![
                "/fixture/hermes",
                "-p",
                "writer",
                "skills",
                "update",
                "@owner/first"
            ],
            vec![
                "/fixture/hermes",
                "-p",
                "writer",
                "skills",
                "update",
                "@owner/second"
            ]
        ]
    );
    assert_eq!(report.failures(), 1);
    assert!(report.lines[0].contains("default/@owner/first") && report.lines[0].contains("exit 1"));
    assert!(
        report.lines[2].contains("writer/@owner/second")
            && report.lines[2].contains("updated detail")
    );
}
#[test]
fn a_held_entry_is_skipped_and_said_so() {
    let (mut roster, config) = setup();
    roster.hermes_registry.get_mut("first").unwrap().held = true;
    let runner = ScriptedRunner::new(&[]);
    let mut report = LaneReport::new("skills");
    roster.update_hermes_registry(&config, &runner, &mut report);
    assert_eq!(
        runner.calls(),
        vec![vec![
            "/fixture/hermes",
            "-p",
            "writer",
            "skills",
            "update",
            "@owner/second"
        ]]
    );
    assert_eq!(report.failures(), 0);
    assert!(
        report
            .lines
            .iter()
            .any(|s| s.contains("default/first") && s.contains("held"))
    );
    assert!(
        report
            .lines
            .iter()
            .any(|s| s.contains("writer/first") && s.contains("held"))
    );
}
#[test]
fn blocked_output_at_exit_zero_is_a_failure_not_a_success() {
    let (roster, config) = setup();
    for output in ["Blocked by scan", "rEfUsEd by scan"] {
        let runner = ScriptedRunner::new(&[]).answering(output);
        let mut report = LaneReport::new("skills");
        roster.update_hermes_registry(&config, &runner, &mut report);
        assert_eq!(report.failures(), 3, "{output}");
        assert_eq!(runner.calls().len(), 3);
        assert!(report.lines.iter().all(|line| line.contains(output)));
    }
}
