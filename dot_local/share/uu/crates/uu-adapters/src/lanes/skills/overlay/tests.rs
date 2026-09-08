use super::*;
use crate::lanes::skills::tests::{directory, roster, write_roster};
use crate::lanes::skills::{SkillsCandidate, SkillsRoster};
fn setup() -> (SkillsCandidate, SkillsRoster) {
    let root = directory();
    let c = SkillsCandidate {
        home: root.join("home"),
    };
    for name in ["alpha", "beta"] {
        std::fs::create_dir_all(c.agents().join("skills").join(name).join("agents")).unwrap();
    }
    let path = root.join("roster");
    write_roster(&path, &roster());
    (c, SkillsRoster::read(&path).unwrap())
}
#[test]
fn an_on_demand_skill_gets_the_codex_overlay_and_a_core_one_does_not() {
    let (c, r) = setup();
    let alpha = c.agents().join("skills/alpha/agents/openai.yaml");
    let beta = c.agents().join("skills/beta/agents/openai.yaml");
    std::fs::write(&alpha, "interface: upstream\n").unwrap();
    std::fs::write(&beta, format!("interface: core\n{POLICY}")).unwrap();
    c.assert_overlays(&r).unwrap();
    assert_eq!(
        std::fs::read_to_string(&alpha).unwrap(),
        format!("interface: upstream\n{POLICY}")
    );
    assert_eq!(std::fs::read_to_string(&beta).unwrap(), "interface: core\n");
    c.assert_overlays(&r).unwrap();
    assert_eq!(
        std::fs::read_to_string(alpha).unwrap(),
        format!("interface: upstream\n{POLICY}")
    );
}
#[test]
fn an_overlay_never_writes_through_a_foreign_symlink() {
    let (c, r) = setup();
    let outside = directory().join("metadata.yaml");
    std::fs::write(&outside, "foreign content\n").unwrap();
    std::os::unix::fs::symlink(&outside, c.agents().join("skills/alpha/agents/openai.yaml"))
        .unwrap();
    assert!(c.assert_overlays(&r).unwrap_err().contains("symlink"));
    assert_eq!(
        std::fs::read_to_string(outside).unwrap(),
        "foreign content\n"
    );
}
