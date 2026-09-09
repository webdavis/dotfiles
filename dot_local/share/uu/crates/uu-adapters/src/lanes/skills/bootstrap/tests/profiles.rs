use super::*;
fn refused(parent_link: bool, publish: bool) {
    for profile in ["default", "writer"] {
        let f = Fixture::new();
        f.current(false);
        if publish {
            missing(&f);
        }
        let mut value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&f.config.lock).unwrap()).unwrap();
        value["hermesProfiles"]["alpha"] = serde_json::json!([profile]);
        std::fs::write(&f.config.lock, serde_json::to_vec(&value).unwrap()).unwrap();
        let parent = if profile == "default" {
            Path::new(&f.config.hermes).to_path_buf()
        } else {
            Path::new(&f.config.hermes).join("profiles").join(profile)
        };
        let outside = f.root.join("outside");
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(outside.join("sentinel"), "retained").unwrap();
        let target = if parent_link {
            parent.clone()
        } else {
            parent.join("skills")
        };
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&outside, &target).unwrap();
        let before = generation(&f);
        let effects = Effects::new(&f);
        let report = run(&f, &effects);
        assert!(
            report
                .lines
                .iter()
                .any(|s| s.contains("symlink; leaving it")),
            "{:?}",
            report.lines
        );
        assert_eq!(
            std::fs::read_to_string(outside.join("sentinel")).unwrap(),
            "retained"
        );
        assert_eq!(std::fs::read_dir(&outside).unwrap().count(), 1);
        assert_eq!(std::fs::read_link(target).unwrap(), outside);
        if publish {
            assert_ne!(generation(&f), before);
        } else {
            assert_eq!(generation(&f), before);
        }
    }
}
#[test]
fn healthy_bootstrap_refuses_a_symlinked_hermes_profile_parent() {
    refused(true, false);
}
#[test]
fn published_bootstrap_refuses_a_symlinked_hermes_profile_parent() {
    refused(true, true);
}
#[test]
fn healthy_bootstrap_refuses_a_symlinked_hermes_skills_child() {
    refused(false, false);
}
#[test]
fn published_bootstrap_refuses_a_symlinked_hermes_skills_child() {
    refused(false, true);
}
