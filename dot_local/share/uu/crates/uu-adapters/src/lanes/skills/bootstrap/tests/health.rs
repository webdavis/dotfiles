use super::*;
#[test]
fn a_link_that_resolves_to_a_skill_without_its_skill_md_is_reinstalled() {
    let f = Fixture::new();
    f.current(false);
    std::fs::remove_file(f.store.agents.join(".skills-current/skills/alpha/SKILL.md")).unwrap();
    let effects = Effects::new(&f);
    let report = run(&f, &effects);
    assert_eq!(report.failures(), 0, "{:?}", report.lines);
    assert_eq!(effects.installs.borrow().as_slice(), ["alpha"]);
    assert_eq!(
        std::fs::read_to_string(f.store.agents.join("skills/alpha/SKILL.md")).unwrap(),
        "new"
    );
}
#[test]
fn a_wrong_store_link_forces_a_reinstall_without_replacing_existing_delivery() {
    let f = Fixture::new();
    f.current(false);
    missing(&f);
    let wrong = Path::new("../.skills-current/skills/beta");
    std::os::unix::fs::symlink(wrong, f.store.agents.join("skills/alpha")).unwrap();
    let effects = Effects::new(&f);
    let report = run(&f, &effects);
    assert_eq!(report.failures(), 0, "{:?}", report.lines);
    assert_eq!(effects.installs.borrow().as_slice(), ["alpha"]);
    assert_eq!(
        std::fs::read_link(f.store.agents.join("skills/alpha")).unwrap(),
        wrong
    );
    assert_eq!(
        std::fs::read_to_string(f.store.agents.join(".skills-current/skills/alpha/SKILL.md"))
            .unwrap(),
        "new"
    );
}
#[test]
fn an_npx_skill_missing_from_the_published_lock_is_reinstalled() {
    let f = Fixture::new();
    f.current(false);
    let lock = f.store.agents.join(".skills-current/.skill-lock.json");
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&lock).unwrap()).unwrap();
    value["skills"].as_object_mut().unwrap().remove("alpha");
    std::fs::write(lock, serde_json::to_vec(&value).unwrap()).unwrap();
    let effects = Effects::new(&f);
    let report = run(&f, &effects);
    assert_eq!(report.failures(), 0, "{:?}", report.lines);
    assert_eq!(effects.installs.borrow().as_slice(), ["alpha"]);
}
#[test]
fn overlay_only_drift_rebuilds_without_running_an_installer() {
    let f = Fixture::new();
    f.current(false);
    let before = generation(&f);
    let overlay = f
        .store
        .agents
        .join(".skills-current/skills/alpha/agents/openai.yaml");
    std::fs::remove_file(&overlay).unwrap();
    let effects = Effects::new(&f);
    let report = run(&f, &effects);
    assert_eq!(report.failures(), 0, "{:?}", report.lines);
    assert_ne!(generation(&f), before);
    assert!(effects.installs.borrow().is_empty());
    assert!(
        std::fs::read_to_string(overlay)
            .unwrap()
            .contains("allow_implicit_invocation: false")
    );
}

#[test]
fn a_core_skill_with_an_owned_on_demand_overlay_is_rebuilt_without_installing() {
    let f = Fixture::new();
    f.current(false);
    let before = generation(&f);
    let path = f
        .store
        .agents
        .join(".skills-current/skills/beta/agents/openai.yaml");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "policy:\n  allow_implicit_invocation: false\n").unwrap();
    let effects = Effects::new(&f);
    let report = run(&f, &effects);
    assert_eq!(report.failures(), 0, "{:?}", report.lines);
    assert_ne!(generation(&f), before);
    assert!(effects.installs.borrow().is_empty());
    assert!(!path.exists());
}
