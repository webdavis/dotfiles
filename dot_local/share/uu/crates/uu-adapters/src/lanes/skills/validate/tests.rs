use super::*;
use crate::lanes::skills::tests::{directory, roster, write_roster};
fn setup() -> (SkillsCandidate, SkillsRoster) {
    let root = directory();
    let c = SkillsCandidate {
        home: root.join("home"),
    };
    for name in ["alpha", "beta", "gamma"] {
        let skill = c.agents().join("skills").join(name);
        std::fs::create_dir_all(skill.join("agents")).unwrap();
        std::fs::write(skill.join("SKILL.md"), name).unwrap();
        if name != "beta" {
            std::fs::write(
                skill.join("agents/openai.yaml"),
                "policy:\n  allow_implicit_invocation: false\n",
            )
            .unwrap();
        }
    }
    std::fs::create_dir_all(c.agents().join("skills/gamma/.clawhub")).unwrap();
    std::fs::write(c.agents().join("skills/gamma/.clawhub/origin.json"), "{}").unwrap();
    std::fs::write(
        c.agents().join(".skill-lock.json"),
        r#"{"skills":{"alpha":{},"beta":{}}}"#,
    )
    .unwrap();
    let path = root.join("roster");
    write_roster(&path, &roster());
    (c, SkillsRoster::read(&path).unwrap())
}
#[test]
fn a_candidate_missing_a_tracked_skill_or_its_skill_md_fails_validation_and_is_discarded() {
    for absent in [
        "skills/alpha",
        "skills/beta/SKILL.md",
        "skills/gamma/.clawhub/origin.json",
    ] {
        let (c, r) = setup();
        assert!(c.validate(&r, SkillsBuildMode::Full).is_ok());
        let path = c.agents().join(absent);
        std::fs::rename(&path, path.with_extension("held")).unwrap();
        let live = c.home.parent().unwrap().join("current");
        std::fs::write(&live, "published bytes").unwrap();
        let why = c.validate(&r, SkillsBuildMode::Full).unwrap_err();
        assert!(why.contains(absent.split('/').nth(1).unwrap()), "{why}");
        assert!(!c.home.exists());
        assert!(c.home.with_file_name("discarded-home").exists());
        assert_eq!(std::fs::read_to_string(live).unwrap(), "published bytes");
    }
}
#[test]
fn a_candidate_whose_overlays_drifted_fails_validation() {
    for (name, text) in [
        ("alpha", "interface: upstream\n"),
        ("beta", "policy:\n  allow_implicit_invocation: false\n"),
    ] {
        let (c, r) = setup();
        std::fs::write(
            c.agents()
                .join("skills")
                .join(name)
                .join("agents/openai.yaml"),
            text,
        )
        .unwrap();
        let why = c.validate(&r, SkillsBuildMode::Full).unwrap_err();
        assert!(why.contains(name) && why.contains("overlay"), "{why}");
        assert!(!c.home.exists());
    }
}
#[test]
fn a_full_candidate_with_a_delisted_npx_lock_key_is_refused_but_additive_preserves_it() {
    let (c, r) = setup();
    let path = c.agents().join(".skill-lock.json");
    let surplus = r#"{"skills":{"alpha":{},"beta":{},"delisted":{}}}"#;
    std::fs::write(&path, surplus).unwrap();
    assert!(c.validate(&r, SkillsBuildMode::Additive).is_ok());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), surplus);
    let why = c.validate(&r, SkillsBuildMode::Full).unwrap_err();
    assert!(why.contains("lock keys"), "{why}");
    assert!(!c.home.exists());
}
