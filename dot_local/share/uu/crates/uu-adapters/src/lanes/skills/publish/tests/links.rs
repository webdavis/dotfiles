use super::*;
#[test]
fn a_published_candidate_becomes_current_and_every_store_link_resolves_into_it() {
    for existing in [false, true] {
        let (_, store, roster) = fixture();
        if existing {
            live(&store);
        }
        let c = ready(&store, "new", SkillsBuildMode::Full);
        store.publish(&c, &roster).unwrap();
        assert_eq!(
            fs::read_to_string(store.agents.join(".skills-current/skills/alpha/SKILL.md"))
                .unwrap_or_default(),
            "new"
        );
        assert_eq!(
            fs::read_link(store.agents.join("skills/alpha")).ok(),
            Some(target("alpha"))
        );
        assert_eq!(
            fs::read_to_string(store.agents.join("skills/alpha/SKILL.md")).unwrap_or_default(),
            "new"
        );
    }
}
#[test]
fn the_npx_lock_link_points_into_the_current_generation() {
    let (_, store, roster) = fixture();
    let c = ready(&store, "new", SkillsBuildMode::Full);
    store.publish(&c, &roster).unwrap();
    assert_eq!(
        fs::read_link(store.agents.join(".skill-lock.json")).ok(),
        Some(PathBuf::from(".skills-current/.skill-lock.json"))
    );
    assert_eq!(
        fs::read_to_string(store.agents.join(".skill-lock.json")).unwrap_or_default(),
        r#"{"skills":{"alpha":{}}}"#
    );
}
#[test]
fn a_tracked_real_directory_is_replaced_only_after_its_content_is_absorbed() {
    for absorbed in [true, false] {
        let (_, store, roster) = fixture();
        live(&store);
        let real = store.agents.join("skills/alpha");
        skill(&real, "writer-content");
        let c = ready(&store, "new", SkillsBuildMode::Full);
        if absorbed {
            c.absorb_store_entries(&store, &roster).unwrap();
        }
        let warnings = store.publish(&c, &roster).unwrap();
        if absorbed {
            assert_eq!(fs::read_link(&real).ok(), Some(target("alpha")));
            assert_eq!(
                fs::read_to_string(real.join("SKILL.md")).unwrap(),
                "writer-content"
            );
        } else {
            assert!(real.is_dir() && !real.is_symlink());
            assert!(
                warnings
                    .iter()
                    .any(|w| w.contains("alpha") && w.contains("unseen"))
            );
            assert_eq!(
                fs::read_to_string(real.join("SKILL.md")).unwrap(),
                "writer-content"
            );
        }
    }
}
#[test]
fn a_store_writer_that_changes_after_absorption_is_preserved_and_reported() {
    let (_, store, roster) = fixture();
    live(&store);
    let real = store.agents.join("skills/alpha");
    skill(&real, "before");
    let c = ready(&store, "new", SkillsBuildMode::Full);
    c.absorb_store_entries(&store, &roster).unwrap();
    skill(&real, "after");
    let warnings = store.publish(&c, &roster).unwrap();
    assert!(real.is_dir() && !real.is_symlink());
    assert_eq!(fs::read_to_string(real.join("SKILL.md")).unwrap(), "after");
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("alpha") && w.contains("changed"))
    );
}
#[test]
fn additive_publication_preserves_existing_store_entries() {
    let (root, store, roster) = fixture();
    live(&store);
    let real = store.agents.join("skills/alpha");
    skill(&real, "kept");
    let c = ready(&store, "new", SkillsBuildMode::Additive);
    c.absorb_store_entries(&store, &roster).unwrap();
    symlink(target("delisted"), store.agents.join("skills/delisted")).unwrap();
    symlink("/foreign/missing", store.agents.join("skills/foreign")).unwrap();
    fs::write(store.agents.join(".skill-lock.json"), "original-lock").unwrap();
    store.publish(&c, &roster).unwrap();
    assert_eq!(
        fs::read_to_string(store.agents.join(".skills-current/skills/alpha/SKILL.md")).unwrap(),
        "kept"
    );
    assert!(real.is_dir() && !real.is_symlink());
    assert_eq!(fs::read_to_string(real.join("SKILL.md")).unwrap(), "kept");
    assert_eq!(
        fs::read_link(store.agents.join("skills/delisted")).ok(),
        Some(target("delisted"))
    );
    assert_eq!(
        fs::read_link(store.agents.join("skills/foreign")).unwrap(),
        PathBuf::from("/foreign/missing")
    );
    assert_eq!(
        fs::read_to_string(root.join(".agents/.skill-lock.json")).unwrap(),
        "original-lock"
    );
}
