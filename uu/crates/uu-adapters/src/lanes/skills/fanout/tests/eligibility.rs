use super::*;
#[test]
fn every_store_skill_reaches_claude_unless_delivery_is_none() {
    let (root, store, mut roster) = fixture();
    skill(&store.agents.join("skills/foreign"));
    roster.claude_undelivered.insert("alpha".into());
    fanout(&root, &store, &roster, SkillsBuildMode::Full);
    delivered(
        &root.join(".claude/skills/foreign"),
        "../../.agents/skills/foreign",
    );
    assert!(!root.join(".claude/skills/alpha").exists());
}
#[test]
fn a_hermes_profile_row_plants_exactly_the_listed_profiles_and_an_empty_row_plants_none() {
    let (root, store, mut roster) = fixture();
    skill(&store.agents.join("skills/foreign"));
    roster
        .hermes_profiles
        .insert("alpha".into(), vec!["default".into(), "developer".into()]);
    roster.hermes_profiles.insert("foreign".into(), vec![]);
    fanout(&root, &store, &roster, SkillsBuildMode::Full);
    delivered(
        &root.join(".hermes/skills/alpha"),
        "../../.agents/skills/alpha",
    );
    delivered(
        &root.join(".hermes/profiles/developer/skills/alpha"),
        "../../../../.agents/skills/alpha",
    );
    for parent in [
        root.join(".hermes/skills"),
        root.join(".hermes/profiles/developer/skills"),
    ] {
        assert!(!parent.join("foreign").exists());
    }
}
#[test]
fn a_collision_name_is_never_fanned_out_to_hermes() {
    let (root, store, mut roster) = fixture();
    for name in ["humanizer", "hyperframes", "alpha"] {
        skill(&store.agents.join("skills").join(name));
        roster
            .hermes_profiles
            .insert(name.into(), vec!["default".into()]);
    }
    fanout(&root, &store, &roster, SkillsBuildMode::Full);
    delivered(
        &root.join(".hermes/skills/alpha"),
        "../../.agents/skills/alpha",
    );
    for name in ["humanizer", "hyperframes"] {
        assert!(!root.join(".hermes/skills").join(name).exists());
    }
}
#[test]
fn owned_delisted_content_leaves_both_delivery_sets_while_foreign_content_remains_eligible() {
    let (root, store, mut roster) = fixture();
    let old = store.candidate("old").unwrap();
    skill(&old.agents().join("skills/delisted"));
    store
        .mark_ready(&old, "old", "2026-09-07T00:00:00Z", SkillsBuildMode::Full)
        .unwrap();
    fs::rename(old.agents(), store.agents.join(".skills-current")).unwrap();
    skill(&store.agents.join("skills/delisted"));
    skill(&store.agents.join("skills/foreign"));
    for name in ["delisted", "foreign"] {
        roster
            .hermes_profiles
            .insert(name.into(), vec!["default".into()]);
    }
    for path in [root.join(".claude/skills"), root.join(".hermes/skills")] {
        fs::create_dir_all(&path).unwrap();
        symlink("../../.agents/skills/delisted", path.join("delisted")).unwrap();
    }
    let c = store.candidate("new").unwrap();
    skill(&c.agents().join("skills/alpha"));
    fs::write(
        c.agents().join(".skill-lock.json"),
        r#"{"skills":{"alpha":{}}}"#,
    )
    .unwrap();
    store
        .mark_ready(&c, "new", "2026-09-07T00:00:00Z", SkillsBuildMode::Full)
        .unwrap();
    store.publish(&c, &roster).unwrap();
    fanout(&root, &store, &roster, SkillsBuildMode::Full);
    for path in [root.join(".claude/skills"), root.join(".hermes/skills")] {
        assert!(!path.join("delisted").is_symlink());
        delivered(&path.join("foreign"), "../../.agents/skills/foreign");
    }
}
