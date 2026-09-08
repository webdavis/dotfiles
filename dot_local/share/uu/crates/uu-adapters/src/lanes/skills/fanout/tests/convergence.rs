use super::*;
#[test]
fn weekly_fanout_repairs_owned_links_and_prunes_them_in_demapped_profiles() {
    let (root, store, mut roster) = fixture();
    roster
        .hermes_profiles
        .insert("alpha".into(), vec!["default".into()]);
    let default = root.join(".hermes/skills");
    let demapped = root.join(".hermes/profiles/former/skills");
    fs::create_dir_all(&default).unwrap();
    fs::create_dir_all(&demapped).unwrap();
    symlink(store.agents.join("skills/wrong"), default.join("alpha")).unwrap();
    symlink("../../../../.agents/skills/alpha", demapped.join("alpha")).unwrap();
    fs::write(demapped.join("foreign"), "kept").unwrap();
    fanout(&root, &store, &roster, SkillsBuildMode::Full);
    delivered(&default.join("alpha"), "../../.agents/skills/alpha");
    assert!(!demapped.join("alpha").is_symlink());
    assert_eq!(
        fs::read_to_string(demapped.join("foreign")).unwrap(),
        "kept"
    );
}
#[test]
fn foreign_links_and_real_destination_entries_survive_both_fanout_modes() {
    for mode in mode_values() {
        let (root, store, mut roster) = fixture();
        for name in [
            "foreign-link",
            "real-entry",
            "deep-link",
            "wrong",
            "missing",
        ] {
            skill(&store.agents.join("skills").join(name));
        }
        for name in [
            "alpha",
            "foreign-link",
            "real-entry",
            "deep-link",
            "wrong",
            "missing",
        ] {
            roster
                .hermes_profiles
                .insert(name.into(), vec!["default".into()]);
        }
        let dir = root.join(".hermes/skills");
        fs::create_dir_all(&dir).unwrap();
        symlink(
            "/someone-else/.agents/skills/alpha",
            dir.join("foreign-link"),
        )
        .unwrap();
        symlink("../../.agents/skills/alpha/nested", dir.join("deep-link")).unwrap();
        fs::write(dir.join("real-entry"), "foreign bytes").unwrap();
        symlink("../../.agents/skills/old", dir.join("wrong")).unwrap();
        symlink("../../.agents/skills/stale", dir.join("stale")).unwrap();
        fanout(&root, &store, &roster, mode);
        assert_eq!(
            fs::read_link(dir.join("foreign-link")).unwrap(),
            PathBuf::from("/someone-else/.agents/skills/alpha")
        );
        assert_eq!(
            fs::read_link(dir.join("deep-link")).unwrap(),
            PathBuf::from("../../.agents/skills/alpha/nested")
        );
        assert_eq!(
            fs::read_to_string(dir.join("real-entry")).unwrap(),
            "foreign bytes"
        );
        delivered(&dir.join("missing"), "../../.agents/skills/missing");
        if mode == SkillsBuildMode::Additive {
            assert_eq!(
                fs::read_link(dir.join("wrong")).unwrap(),
                PathBuf::from("../../.agents/skills/old")
            );
            assert!(dir.join("stale").is_symlink());
        } else {
            delivered(&dir.join("wrong"), "../../.agents/skills/wrong");
            assert!(!dir.join("stale").is_symlink());
        }
    }
}
