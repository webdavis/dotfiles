mod health;
mod preservation;
mod profiles;
use crate::lanes::skills::tests::fixture::{Effects, Fixture, skill};
use std::path::Path;
fn generation(f: &Fixture) -> Vec<u8> {
    std::fs::read(f.store.agents.join(".skills-current/generation.json")).unwrap()
}
fn run(f: &Fixture, effects: &Effects) -> uu_domain::LaneReport {
    f.config.bootstrap_skills("skills", &f.root, effects)
}
fn missing(f: &Fixture) {
    std::fs::remove_file(f.store.agents.join("skills/alpha")).unwrap();
}
#[test]
fn a_healthy_store_bootstraps_as_a_no_op_that_publishes_nothing() {
    let f = Fixture::new();
    f.current(false);
    let before = generation(&f);
    let effects = Effects::new(&f);
    let report = run(&f, &effects);
    assert_eq!(report.failures(), 0, "{:?}", report.lines);
    assert_eq!(generation(&f), before);
    assert!(effects.installs.borrow().is_empty());
    assert_eq!(effects.calls.borrow().as_slice(), ["routing --check"]);
    assert!(report.lines.iter().any(|s| s.contains("healthy")));
}
#[test]
fn an_absent_roster_skill_is_installed_and_published() {
    let f = Fixture::new();
    f.current(false);
    let before = generation(&f);
    missing(&f);
    let effects = Effects::new(&f);
    let report = run(&f, &effects);
    assert_eq!(report.failures(), 0, "{:?}", report.lines);
    assert_ne!(generation(&f), before);
    assert_eq!(
        std::fs::read_to_string(f.store.agents.join("skills/alpha/SKILL.md")).unwrap(),
        "new"
    );
    assert_eq!(effects.installs.borrow().as_slice(), ["alpha"]);
}
#[test]
fn bootstrap_never_updates_a_present_and_healthy_skill() {
    let f = Fixture::new();
    f.current(false);
    missing(&f);
    let effects = Effects::new(&f);
    let report = run(&f, &effects);
    assert_eq!(report.failures(), 0, "{:?}", report.lines);
    assert_eq!(effects.installs.borrow().as_slice(), ["alpha"]);
    for name in ["beta", "gamma"] {
        assert_eq!(
            std::fs::read_to_string(f.store.agents.join("skills").join(name).join("SKILL.md"))
                .unwrap(),
            "old"
        );
    }
}
#[test]
fn bootstrap_never_migrates_a_flat_store() {
    let f = Fixture::new();
    for name in ["alpha", "beta", "gamma"] {
        skill(&f.store.agents.join("skills").join(name), "flat");
    }
    let effects = Effects::new(&f);
    let report = run(&f, &effects);
    assert_eq!(report.failures(), 0, "{:?}", report.lines);
    assert!(!f.store.agents.join(".skills-current").exists());
    assert!(effects.installs.borrow().is_empty());
    for name in ["alpha", "beta", "gamma"] {
        let path = f.store.agents.join("skills").join(name);
        assert!(!path.is_symlink());
        assert_eq!(
            std::fs::read_to_string(path.join("SKILL.md")).unwrap(),
            "flat"
        );
    }
    assert!(
        f.store
            .agents
            .join("skills/alpha/agents/openai.yaml")
            .is_file()
    );
    assert!(effects.calls.borrow().contains(&"routing --check".into()));
}
#[test]
fn bootstrap_returns_failure_when_a_required_post_publish_check_fails() {
    let f = Fixture::new();
    f.current(false);
    missing(&f);
    let mut effects = Effects::new(&f);
    effects.fail_routing = true;
    let report = run(&f, &effects);
    assert!(report.failures() > 0);
    assert!(
        report
            .lines
            .iter()
            .any(|s| s.contains("fixture routing failure"))
    );
    assert_eq!(effects.installs.borrow().as_slice(), ["alpha"]);
}
