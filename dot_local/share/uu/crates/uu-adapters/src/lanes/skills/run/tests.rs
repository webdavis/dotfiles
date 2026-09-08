mod fixture;
mod publication;
use fixture::{Effects, Fixture, skill};
#[test]
fn a_weekly_skills_run_executes_every_required_phase_and_reports_its_outcome() {
    let f = Fixture::new();
    f.current(false);
    let effects = Effects::new(&f);
    let report = f.config.run_skills("mine", &f.root, &effects);
    assert_eq!(report.name, "mine");
    assert_eq!(report.failures(), 0, "{:?}", report.lines);
    assert_eq!(
        effects.calls.borrow().as_slice(),
        [
            "npx",
            "clawhub",
            "cua-driver skills update",
            "routing --check",
            "hermes -p default skills update @owner/hub",
            "fork comparison",
            "fork comparison",
            "fork comparison"
        ]
    );
    let said = report.lines.join("\n");
    for phase in [
        "recovery",
        "generation published",
        "cua-driver",
        "fan-out",
        "overlays",
        "routing",
        "hermes default/@owner/hub",
        "fork-drift",
        "npx-tracked skills",
        "clawhub-tracked skills",
    ] {
        assert!(said.contains(phase), "missing {phase}: {said}");
    }
    assert!(
        f.store
            .agents
            .join(".skills-current/skills/alpha/agents/openai.yaml")
            .is_file()
    );
}
#[test]
fn a_flat_store_is_migrated_and_recovered_before_weekly_candidate_build() {
    let f = Fixture::new();
    for name in ["alpha", "beta", "gamma"] {
        skill(&f.store.agents.join("skills").join(name), "flat");
    }
    std::fs::write(f.store.agents.join(".skill-lock.json"), "{\"skills\":{}}").unwrap();
    let effects = Effects::new(&f);
    let report = f.config.run_skills("skills", &f.root, &effects);
    assert_eq!(report.failures(), 0, "{:?}", report.lines);
    assert!(
        *effects.migrated.borrow(),
        "migration completed before installer"
    );
    assert!(
        report
            .lines
            .iter()
            .position(|s| s.contains("flat-store migration"))
            .unwrap()
            < report
                .lines
                .iter()
                .rposition(|s| s.contains("recovery"))
                .unwrap()
    );
    assert!(f.store.agents.join(".skill-lock.json").is_symlink());
}
#[test]
fn a_failed_candidate_leaves_publication_untouched_but_still_runs_live_followup_phases() {
    let f = Fixture::new();
    f.current(false);
    let original = std::fs::read(f.store.agents.join(".skills-current/generation.json")).unwrap();
    let mut effects = Effects::new(&f);
    effects.fail_candidate = true;
    let report = f.config.run_skills("skills", &f.root, &effects);
    assert!(
        report.failures() > 0
            && report
                .lines
                .iter()
                .any(|s| s.contains("fixture npx failure"))
    );
    assert_eq!(
        std::fs::read(f.store.agents.join(".skills-current/generation.json")).unwrap(),
        original
    );
    for expected in [
        "clawhub",
        "cua-driver skills update",
        "routing --check",
        "hermes -p default skills update @owner/hub",
        "fork comparison",
    ] {
        assert!(effects.calls.borrow().iter().any(|s| s == expected));
    }
}
#[test]
fn weekly_before_and_after_fingerprints_produce_the_skills_change_section() {
    let f = Fixture::new();
    f.current(false);
    let effects = Effects::new(&f);
    let report = f.config.run_skills("skills", &f.root, &effects);
    let said = report.lines.join("\n");
    assert!(
        said.contains("npx-tracked skills: 2 of 2") && said.contains("`alpha` `old` -> `new`"),
        "{said}"
    );
    assert!(
        said.contains("clawhub-tracked skills: 1 of 1") && said.contains("`gamma` `old` -> `new`"),
        "{said}"
    );
}
#[test]
fn weekly_skills_execution_creates_missing_hermes_destinations() {
    let f = Fixture::new();
    f.current(false);
    let effects = Effects::new(&f);
    let report = f.config.run_skills("skills", &f.root, &effects);
    assert_eq!(report.failures(), 0);
    assert_eq!(
        std::fs::read_to_string(
            std::path::Path::new(&f.config.hermes).join("skills/alpha/SKILL.md")
        )
        .unwrap(),
        "new"
    );
}
#[test]
fn an_unreadable_skills_fingerprint_is_reported_without_stopping_the_weekly_run() {
    let f = Fixture::new();
    f.current(false);
    std::fs::write(
        f.store
            .agents
            .join(".skills-current/skills/gamma/.clawhub/origin.json"),
        "broken",
    )
    .unwrap();
    let effects = Effects::new(&f);
    let report = f.config.run_skills("skills", &f.root, &effects);
    assert_eq!(report.failures(), 0, "{:?}", report.lines);
    let said = report.lines.join("\n");
    assert!(
        said.contains("clawhub-tracked skills: NOT COMPARED"),
        "{said}"
    );
    assert!(effects.calls.borrow().contains(&"routing --check".into()));
}
