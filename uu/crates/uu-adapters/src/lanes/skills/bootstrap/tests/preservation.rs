use super::*;
#[test]
fn bootstrap_preserves_delisted_generation_lock_store_and_delivery_entries() {
    let f = Fixture::new();
    f.current(true);
    missing(&f);
    for path in [
        Path::new(&f.config.claude_skills).to_path_buf(),
        Path::new(&f.config.hermes).join("skills"),
    ] {
        std::os::unix::fs::symlink("../../.agents/skills/stale", path.join("stale")).unwrap();
        std::os::unix::fs::symlink("../../.agents/skills/beta", path.join("alpha")).unwrap();
    }
    let effects = Effects::new(&f);
    let report = run(&f, &effects);
    assert_eq!(report.failures(), 0, "{:?}", report.lines);
    assert_eq!(effects.installs.borrow().as_slice(), ["alpha"]);
    assert!(
        report
            .lines
            .iter()
            .any(|s| s.contains("generation published"))
    );
    for name in ["delisted", "owned-real"] {
        assert!(
            f.store
                .agents
                .join(".skills-current/skills")
                .join(name)
                .join("SKILL.md")
                .is_file()
        );
        assert!(
            f.store
                .agents
                .join("skills")
                .join(name)
                .join("SKILL.md")
                .is_file()
        );
    }
    let lock: serde_json::Value = serde_json::from_slice(
        &std::fs::read(f.store.agents.join(".skills-current/.skill-lock.json")).unwrap(),
    )
    .unwrap();
    assert!(lock["skills"].get("delisted").is_some());
    assert_eq!(
        std::fs::read_to_string(f.store.agents.join("skills/owned-real/SKILL.md")).unwrap(),
        "operator edit"
    );
    assert_eq!(
        std::fs::read_to_string(f.store.agents.join("skills/foreign/SKILL.md")).unwrap(),
        "foreign"
    );
    for path in [
        Path::new(&f.config.claude_skills).to_path_buf(),
        Path::new(&f.config.hermes).join("skills"),
    ] {
        assert_eq!(
            std::fs::read_link(path.join("stale")).unwrap(),
            Path::new("../../.agents/skills/stale")
        );
        assert_eq!(
            std::fs::read_link(path.join("alpha")).unwrap(),
            Path::new("../../.agents/skills/beta")
        );
        assert!(path.join("delisted").is_symlink());
    }
}
#[test]
fn healthy_bootstrap_creates_missing_hermes_destinations_without_publishing() {
    let f = Fixture::new();
    f.current(false);
    let before = generation(&f);
    let effects = Effects::new(&f);
    let report = run(&f, &effects);
    assert_eq!(report.failures(), 0, "{:?}", report.lines);
    assert_eq!(generation(&f), before);
    assert_eq!(
        std::fs::read_to_string(Path::new(&f.config.hermes).join("skills/alpha/SKILL.md")).unwrap(),
        "old"
    );
}
#[test]
fn additive_publish_creates_missing_hermes_destinations_without_replacing_existing_links() {
    let f = Fixture::new();
    f.current(false);
    missing(&f);
    let claude = Path::new(&f.config.claude_skills);
    std::fs::create_dir_all(claude).unwrap();
    std::os::unix::fs::symlink("operator-target", claude.join("alpha")).unwrap();
    let effects = Effects::new(&f);
    let report = run(&f, &effects);
    assert_eq!(report.failures(), 0, "{:?}", report.lines);
    assert_eq!(
        std::fs::read_to_string(Path::new(&f.config.hermes).join("skills/alpha/SKILL.md")).unwrap(),
        "new"
    );
    assert_eq!(
        std::fs::read_link(claude.join("alpha")).unwrap(),
        Path::new("operator-target")
    );
}
