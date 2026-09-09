use super::*;
#[test]
fn missing_hermes_profile_and_skills_directories_are_created_before_link_delivery() {
    for mode in mode_values() {
        let (root, store, mut roster) = fixture();
        roster
            .hermes_profiles
            .insert("alpha".into(), vec!["default".into(), "new-profile".into()]);
        fanout(&root, &store, &roster, mode);
        for (path, prefix) in [
            (root.join(".hermes/skills"), "../../.agents/skills/alpha"),
            (
                root.join(".hermes/profiles/new-profile/skills"),
                "../../../../.agents/skills/alpha",
            ),
        ] {
            assert!(path.is_dir() && !path.is_symlink());
            assert!(path.parent().unwrap().is_dir());
            delivered(&path.join("alpha"), prefix);
        }
    }
}
fn guarded(parent_link: bool) {
    for mode in mode_values() {
        for named in [false, true] {
            let (root, store, mut roster) = fixture();
            let profile = if named { "developer" } else { "default" };
            roster
                .hermes_profiles
                .insert("alpha".into(), vec![profile.into()]);
            let parent = if named {
                root.join(".hermes/profiles/developer")
            } else {
                root.join(".hermes")
            };
            let outside = root.join("outside");
            fs::create_dir_all(&outside).unwrap();
            fs::write(outside.join("sentinel"), "untouched").unwrap();
            let link_dir = if parent_link {
                outside.join("skills")
            } else {
                outside.clone()
            };
            fs::create_dir_all(&link_dir).unwrap();
            let prefix = if named {
                "../../../../.agents/skills"
            } else {
                "../../.agents/skills"
            };
            symlink(format!("{prefix}/stale"), link_dir.join("stale")).unwrap();
            if parent_link {
                fs::create_dir_all(parent.parent().unwrap()).unwrap();
                symlink(&outside, &parent).unwrap();
            } else {
                fs::create_dir_all(&parent).unwrap();
                symlink(&outside, parent.join("skills")).unwrap();
            }
            let warnings = fanout(&root, &store, &roster, mode);
            assert_eq!(
                fs::read_to_string(outside.join("sentinel")).unwrap(),
                "untouched"
            );
            let mut names = fs::read_dir(&link_dir)
                .unwrap()
                .map(|e| e.unwrap().file_name())
                .collect::<Vec<_>>();
            names.sort();
            let expected = if parent_link {
                vec![std::ffi::OsString::from("stale")]
            } else {
                vec!["sentinel".into(), "stale".into()]
            };
            assert_eq!(names, expected);
            assert_eq!(
                fs::read_link(link_dir.join("stale")).unwrap(),
                PathBuf::from(format!("{prefix}/stale"))
            );
            assert!(warnings.iter().any(|w| w.contains("symlink")));
        }
    }
}
#[test]
fn a_profile_parent_that_is_a_symlink_is_refused() {
    guarded(true);
}
#[test]
fn a_profile_skills_dir_that_is_a_symlink_is_refused() {
    guarded(false);
}
