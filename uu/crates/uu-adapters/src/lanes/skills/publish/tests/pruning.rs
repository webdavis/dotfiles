use super::*;
#[test]
fn a_delisted_managed_store_link_is_removed_without_following_foreign_symlinks() {
    let (root, store, roster) = fixture();
    live(&store);
    let outside = root.join("outside");
    skill(&outside, "sentinel");
    symlink(target("delisted"), store.agents.join("skills/delisted")).unwrap();
    symlink(&outside, store.agents.join("skills/foreign")).unwrap();
    symlink(
        "../.skills-current/skills/another",
        store.agents.join("skills/wrong-name"),
    )
    .unwrap();
    symlink(target("alpha"), store.agents.join("skills/alpha")).unwrap();
    let c = ready(&store, "new", SkillsBuildMode::Full);
    store.publish(&c, &roster).unwrap();
    assert!(!store.agents.join("skills/delisted").is_symlink());
    assert!(store.agents.join("skills/foreign").is_symlink());
    assert!(store.agents.join("skills/wrong-name").is_symlink());
    assert_eq!(
        fs::read_to_string(outside.join("SKILL.md")).unwrap(),
        "sentinel"
    );
    assert!(store.agents.join("skills/alpha").is_symlink());
}
#[test]
fn a_delisted_outgoing_owned_real_directory_leaves_the_store() {
    let (_, store, roster) = fixture();
    live(&store);
    skill(&store.agents.join("skills/delisted"), "clobbered");
    let c = ready(&store, "new", SkillsBuildMode::Full);
    store.publish(&c, &roster).unwrap();
    assert!(!store.agents.join("skills/delisted").exists());
    assert_eq!(
        fs::read_to_string(
            store
                .agents
                .join(".skills-quarantine/new/delisted/SKILL.md")
        )
        .unwrap_or_default(),
        "clobbered"
    );
}
#[test]
fn a_foreign_real_directory_survives_delisted_pruning() {
    let (_, store, roster) = fixture();
    live(&store);
    skill(&store.agents.join("skills/foreign"), "foreign");
    let c = ready(&store, "new", SkillsBuildMode::Full);
    store.publish(&c, &roster).unwrap();
    assert_eq!(
        fs::read_to_string(store.agents.join("skills/foreign/SKILL.md")).unwrap_or_default(),
        "foreign"
    );
    assert_eq!(
        fs::read_to_string(store.agents.join("skills/alpha/SKILL.md")).unwrap_or_default(),
        "new"
    );
}
