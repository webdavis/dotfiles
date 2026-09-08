use super::*;
#[test]
fn recovery_after_exchange_prunes_with_outgoing_names_before_sweeping() {
    let (_, store, roster) = fixture();
    live(&store);
    skill(&store.agents.join("skills/delisted"), "clobbered");
    let c = ready(&store, "new", SkillsBuildMode::Full);
    store.prepare_publish(&c).unwrap();
    crate::lanes::skills::exchange_skills_directories(
        &c.agents(),
        &store.agents.join(".skills-current"),
    )
    .unwrap();
    store.recover_publish(&roster).unwrap();
    assert!(!store.agents.join("skills/delisted").exists());
    assert!(!store.agents.join(".skills-exchange.json").exists());
    assert_eq!(
        fs::read_to_string(
            store
                .agents
                .join(".skills-generations/old/skills/delisted/SKILL.md")
        )
        .unwrap_or_default(),
        "outgoing"
    );
    assert!(!c.home.exists());
}
#[test]
fn reused_candidates_pass_e9_validation_before_any_exchange() {
    let (_, store, roster) = fixture();
    live(&store);
    let c = ready(&store, "new", SkillsBuildMode::Full);
    fs::rename(
        c.agents().join("skills/alpha/SKILL.md"),
        c.home.join("missing-skill.md"),
    )
    .unwrap();
    assert!(
        store
            .recover_publish(&roster)
            .unwrap_err()
            .contains("SKILL.md")
    );
    assert_eq!(
        fs::read_to_string(store.agents.join(".skills-current/skills/alpha/SKILL.md")).unwrap(),
        "old"
    );
    assert!(!c.home.exists());
    assert!(!store.agents.join(".skills-exchange.json").exists());
}
#[test]
fn interrupted_publication_validates_the_published_content_and_retains_ownership_on_failure() {
    let (_, store, roster) = fixture();
    live(&store);
    let c = ready(&store, "new", SkillsBuildMode::Full);
    store.prepare_publish(&c).unwrap();
    crate::lanes::skills::exchange_skills_directories(
        &c.agents(),
        &store.agents.join(".skills-current"),
    )
    .unwrap();
    fs::write(
        store.agents.join(".skills-current/.skill-lock.json"),
        r#"{"skills":{"alpha":{},"delisted":{}}}"#,
    )
    .unwrap();
    assert!(
        store
            .recover_publish(&roster)
            .unwrap_err()
            .contains("lock keys")
    );
    assert!(store.agents.join(".skills-exchange.json").exists());
    assert_eq!(
        fs::read_to_string(c.agents().join("skills/delisted/SKILL.md")).unwrap(),
        "outgoing"
    );
}
#[test]
fn failed_pruning_keeps_the_journal_and_resumes_cleanup_on_restart() {
    let (_, store, roster) = fixture();
    live(&store);
    skill(&store.agents.join("skills/delisted"), "clobbered");
    fs::write(store.agents.join(".skills-quarantine"), "blocked").unwrap();
    let c = ready(&store, "new", SkillsBuildMode::Full);
    assert!(store.publish(&c, &roster).is_err());
    let marker = store.agents.join(".skills-exchange.json");
    assert!(marker.exists());
    assert!(fs::read_to_string(&marker).unwrap().contains("delisted"));
    assert!(
        store
            .agents
            .join(".skills-generations/old/skills/delisted")
            .exists()
    );
    fs::rename(
        store.agents.join(".skills-quarantine"),
        store.agents.join("quarantine-blocker"),
    )
    .unwrap();
    store.recover_publish(&roster).unwrap();
    assert!(!store.agents.join("skills/delisted").exists());
    assert!(!marker.exists());
    assert!(!c.home.exists());
}
#[test]
fn interrupted_workspace_cleanup_keeps_ownership_until_a_successful_retry() {
    use std::os::unix::fs::PermissionsExt;
    let (_, store, roster) = fixture();
    live(&store);
    let c = ready(&store, "new", SkillsBuildMode::Full);
    let garbage = store
        .agents
        .join(".skills-generations/earlier.garbage.1/blocked");
    fs::create_dir_all(&garbage).unwrap();
    fs::write(garbage.join("kept"), "pending").unwrap();
    fs::set_permissions(&garbage, fs::Permissions::from_mode(0o000)).unwrap();
    let failed = store.publish(&c, &roster);
    fs::set_permissions(&garbage, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(failed.is_err());
    assert!(store.agents.join(".skills-exchange.json").exists());
    assert!(
        store
            .agents
            .join(".skills-generations/old/skills/delisted")
            .exists()
    );
    store.recover_publish(&roster).unwrap();
    assert!(!garbage.exists());
    assert!(!store.agents.join(".skills-exchange.json").exists());
}
