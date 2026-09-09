use super::*;
fn assert_pruned(f: &Fixture) {
    for name in ["delisted", "owned-real"] {
        assert!(
            !f.store
                .agents
                .join(".skills-current/skills")
                .join(name)
                .exists()
        );
        assert!(
            !f.store.agents.join("skills").join(name).exists()
                && !f.store.agents.join("skills").join(name).is_symlink()
        );
        assert!(
            !std::path::Path::new(&f.config.claude_skills)
                .join(name)
                .is_symlink()
        );
        assert!(
            !std::path::Path::new(&f.config.hermes)
                .join("skills")
                .join(name)
                .is_symlink()
        );
    }
    assert_eq!(
        std::fs::read_to_string(f.store.agents.join("skills/foreign/SKILL.md")).unwrap(),
        "foreign"
    );
    assert!(
        std::path::Path::new(&f.config.claude_skills)
            .join("foreign/SKILL.md")
            .is_file()
    );
    assert!(
        std::path::Path::new(&f.config.hermes)
            .join("skills/alpha/SKILL.md")
            .is_file()
    );
}
#[test]
fn fresh_weekly_publication_prunes_owned_delisted_content_before_delivery() {
    let f = Fixture::new();
    f.current(true);
    let effects = Effects::new(&f);
    let report = f.config.run_skills("skills", &f.root, &effects);
    assert_eq!(report.failures(), 0, "{:?}", report.lines);
    assert_pruned(&f);
    assert!(effects.calls.borrow().contains(&"npx".into()));
}
#[test]
fn reused_weekly_publication_prunes_owned_delisted_content_before_delivery() {
    let f = Fixture::new();
    f.current(true);
    let _ = f.candidate("reused", "new", false);
    let effects = Effects::new(&f);
    let report = f.config.run_skills("skills", &f.root, &effects);
    assert_eq!(report.failures(), 0, "{:?}", report.lines);
    assert_pruned(&f);
    assert!(!effects.calls.borrow().contains(&"npx".into()));
    assert!(report.lines.iter().any(|s| s.contains("reused")));
}
#[test]
fn a_refused_publication_recovery_preserves_its_journal_and_skips_another_build() {
    let f = Fixture::new();
    f.current(false);
    let candidate = f.candidate("interrupted", "new", false);
    f.store.prepare_publish(&candidate).unwrap();
    std::fs::remove_file(candidate.agents().join("skills/alpha/SKILL.md")).unwrap();
    let before = std::fs::read(f.store.agents.join(".skills-current/generation.json")).unwrap();
    let effects = Effects::new(&f);
    let report = f.config.run_skills("skills", &f.root, &effects);
    assert!(report.failures() > 0);
    assert!(!effects.calls.borrow().contains(&"npx".into()));
    assert!(f.store.agents.join(".skills-exchange.json").is_file());
    assert_eq!(
        std::fs::read(f.store.agents.join(".skills-current/generation.json")).unwrap(),
        before
    );
    assert!(effects.calls.borrow().contains(&"routing --check".into()));
}
