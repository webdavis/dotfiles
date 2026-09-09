use super::*;
use crate::lanes::skills::tests::directory;
fn fixture() -> (PathBuf, SkillsGenerationStore) {
    let root = directory();
    let exe = root.join("uu");
    std::fs::write(&exe, "code-v1").unwrap();
    let store = SkillsGenerationStore::capture(&root.join("agents"), "roster-v1", &exe).unwrap();
    (root, store)
}
fn ready(store: &SkillsGenerationStore, id: &str, mode: SkillsBuildMode) -> SkillsCandidate {
    let c = store.candidate(id).unwrap();
    std::fs::write(c.agents().join("skills").join("owned-name"), "content").unwrap();
    store
        .mark_ready(&c, id, "2026-09-07T00:00:00Z", mode)
        .unwrap();
    c
}
fn live(store: &SkillsGenerationStore) {
    let c = ready(store, "old", SkillsBuildMode::Full);
    std::fs::rename(c.agents(), store.agents.join(".skills-current")).unwrap();
}
fn meta(path: &Path, id: &str) {
    std::fs::create_dir_all(path).unwrap();
    std::fs::write(path.join("generation.json"),serde_json::json!({"id":id,"createdAt":"2026-09-07T00:00:00Z","customLockHash":"roster-v1","updaterHash":"old-code","buildMode":"full"}).to_string()).unwrap();
}
#[test]
fn an_exchange_swaps_two_directories_atomically_and_both_paths_keep_resolving() {
    let root = directory();
    for n in ["a", "b"] {
        std::fs::create_dir(root.join(n)).unwrap();
        std::fs::write(root.join(n).join("value"), n).unwrap();
    }
    exchange_skills_directories(&root.join("a"), &root.join("b")).unwrap();
    assert_eq!(std::fs::read_to_string(root.join("a/value")).unwrap(), "b");
    assert_eq!(std::fs::read_to_string(root.join("b/value")).unwrap(), "a");
    let finished = std::sync::atomic::AtomicBool::new(false);
    let barrier = std::sync::Barrier::new(2);
    std::thread::scope(|scope| {
        let reader = scope.spawn(|| {
            barrier.wait();
            while !finished.load(std::sync::atomic::Ordering::Acquire) {
                for path in [root.join("a/value"), root.join("b/value")] {
                    let value = std::fs::read_to_string(path)
                        .expect("an exchange must leave every lookup resolving");
                    assert!(value == "a" || value == "b");
                }
            }
        });
        barrier.wait();
        for _ in 0..8 {
            exchange_skills_directories(&root.join("a"), &root.join("b")).unwrap();
        }
        finished.store(true, std::sync::atomic::Ordering::Release);
        reader.join().unwrap();
    });
}
#[test]
fn a_candidate_without_its_ready_marker_is_incomplete_and_never_published() {
    let (_, s) = fixture();
    live(&s);
    let c = s.candidate("new").unwrap();
    assert!(s.prepare_publish(&c).unwrap_err().contains("metadata"));
    assert!(!s.agents.join(".skills-exchange.json").exists());
    assert!(s.agents.join(".skills-current/skills/owned-name").exists());
}
#[test]
fn exactly_one_previous_generation_is_retained_and_older_ones_are_swept() {
    let (_, s) = fixture();
    for id in ["a", "b", "c"] {
        meta(&s.agents.join(".skills-generations").join(id), id);
    }
    s.sweep("b").unwrap();
    assert!(
        s.agents
            .join(".skills-generations/b/generation.json")
            .exists()
    );
    for id in ["a", "c"] {
        assert!(!s.agents.join(".skills-generations").join(id).exists());
    }
}
#[test]
fn a_recovered_complete_candidate_built_from_the_current_roster_is_reused() {
    let (_, s) = fixture();
    let c = ready(&s, "new", SkillsBuildMode::Full);
    let SkillsRecovery::Candidate(got) = s.recover().unwrap() else {
        panic!("compatible candidate was not recovered")
    };
    assert_eq!(got.home, c.home);
}
#[test]
fn an_additive_bootstrap_candidate_is_never_reused_as_a_full_weekly_refresh() {
    let (_, s) = fixture();
    let c = ready(&s, "new", SkillsBuildMode::Additive);
    assert!(c.agents().join("generation.json").exists());
    assert!(matches!(s.recover().unwrap(), SkillsRecovery::None));
    assert!(c.home.exists());
}
#[test]
fn different_updater_content_at_the_same_package_version_refuses_candidate_reuse() {
    let (root, s) = fixture();
    let c = ready(&s, "new", SkillsBuildMode::Full);
    std::fs::write(root.join("uu"), "code-v2").unwrap();
    let restarted =
        SkillsGenerationStore::capture(&s.agents, "roster-v1", &root.join("uu")).unwrap();
    assert_ne!(s.updater_hash, restarted.updater_hash);
    assert!(matches!(restarted.recover().unwrap(), SkillsRecovery::None));
    assert!(c.home.exists());
}
#[test]
fn replacement_after_startup_does_not_change_the_captured_updater_identity() {
    let (root, s) = fixture();
    let initial = s.updater_hash.clone();
    std::fs::write(root.join("uu"), "code-v2").unwrap();
    let c = ready(&s, "new", SkillsBuildMode::Full);
    let v: serde_json::Value =
        serde_json::from_slice(&std::fs::read(c.agents().join("generation.json")).unwrap())
            .unwrap();
    assert_eq!(v["updaterHash"], initial);
    let restarted =
        SkillsGenerationStore::capture(&s.agents, "roster-v1", &root.join("uu")).unwrap();
    assert_ne!(v["updaterHash"], restarted.updater_hash);
}
#[test]
fn a_generation_whose_metadata_id_disagrees_with_its_directory_is_refused() {
    let (_, s) = fixture();
    let c = ready(&s, "new", SkillsBuildMode::Full);
    let p = c.agents().join("generation.json");
    let mut v: serde_json::Value = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
    v["id"] = "elsewhere".into();
    std::fs::write(p, v.to_string()).unwrap();
    assert!(matches!(s.recover().unwrap(), SkillsRecovery::None));
    assert!(c.home.exists());
}
#[test]
fn an_in_flight_exchange_marker_is_finished_on_the_next_run() {
    let (_, s) = fixture();
    live(&s);
    let c = ready(&s, "new", SkillsBuildMode::Full);
    s.prepare_publish(&c).unwrap();
    assert!(s.agents.join(".skills-exchange.json").exists());
    let SkillsRecovery::Publication(p) = s.recover().unwrap() else {
        panic!("publication not recovered")
    };
    assert_eq!(p.previous_id, "old");
    assert!(
        s.agents
            .join(".skills-generations/old/generation.json")
            .exists()
    );
    let v: serde_json::Value = serde_json::from_slice(
        &std::fs::read(s.agents.join(".skills-current/generation.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(v["id"], "new");
}
#[test]
fn failed_prior_generation_retention_keeps_the_marker_and_workspace_out_of_the_sweep() {
    let (_, s) = fixture();
    live(&s);
    let c = ready(&s, "new", SkillsBuildMode::Full);
    s.prepare_publish(&c).unwrap();
    std::fs::write(s.agents.join(".skills-generations/old/blocker"), "foreign").unwrap();
    assert!(s.recover().is_err());
    assert!(s.sweep("old").is_err());
    assert!(s.agents.join(".skills-exchange.json").exists());
    assert!(c.agents().join("skills/owned-name").exists());
    assert_eq!(
        std::fs::read_to_string(s.agents.join(".skills-generations/old/blocker")).unwrap(),
        "foreign"
    );
}
#[test]
fn recovery_keeps_outgoing_ownership_evidence_until_delisted_pruning_finishes() {
    let (_, s) = fixture();
    live(&s);
    let c = ready(&s, "new", SkillsBuildMode::Full);
    s.prepare_publish(&c).unwrap();
    exchange_skills_directories(&c.agents(), &s.agents.join(".skills-current")).unwrap();
    let SkillsRecovery::Publication(p) = s.recover().unwrap() else {
        panic!("post-exchange publication not recovered")
    };
    assert_eq!(p.outgoing_names, BTreeSet::from(["owned-name".into()]));
    let SkillsRecovery::Publication(again) = s.recover().unwrap() else {
        panic!("ownership lost on replay")
    };
    assert_eq!(again.outgoing_names, p.outgoing_names);
    assert!(s.agents.join(".skills-exchange.json").exists());
    s.finish_publication(&p.previous_id).unwrap();
    assert!(!s.agents.join(".skills-exchange.json").exists());
}

#[test]
fn publication_cleanup_resumes_after_its_empty_workspace_was_already_removed() {
    let (_, s) = fixture();
    live(&s);
    let c = ready(&s, "new", SkillsBuildMode::Full);
    s.prepare_publish(&c).unwrap();
    let SkillsRecovery::Publication(p) = s.recover().unwrap() else {
        panic!("publication not recovered")
    };
    std::fs::remove_dir(&c.home).unwrap();
    std::fs::remove_dir(c.home.parent().unwrap()).unwrap();
    s.finish_publication(&p.previous_id).unwrap();
    assert!(!s.agents.join(".skills-exchange.json").exists());
    assert!(
        s.agents
            .join(".skills-generations/old/generation.json")
            .exists()
    );
}

#[test]
fn publication_cleanup_sweeps_the_owned_installer_workspace_after_pruning() {
    let (_, s) = fixture();
    live(&s);
    let c = ready(&s, "new", SkillsBuildMode::Full);
    std::fs::create_dir_all(c.home.join(".cache/npm")).unwrap();
    std::fs::write(c.home.join(".cache/npm/owned"), "installer cache").unwrap();
    s.prepare_publish(&c).unwrap();
    let SkillsRecovery::Publication(p) = s.recover().unwrap() else {
        panic!("publication not recovered")
    };
    assert!(c.home.join(".cache/npm/owned").exists());
    s.finish_publication(&p.previous_id).unwrap();
    assert!(!c.home.parent().unwrap().exists());
    assert!(!s.agents.join(".skills-exchange.json").exists());
    assert!(s.agents.join(".skills-current/skills/owned-name").exists());
    assert!(
        s.agents
            .join(".skills-generations/old/generation.json")
            .exists()
    );
}
#[test]
fn an_interrupted_garbage_sweep_finishes_without_reusing_its_tree() {
    let (_, s) = fixture();
    let generations = s.agents.join(".skills-generations");
    meta(&generations.join("keep"), "keep");
    meta(&generations.join("old.garbage.123"), "old");
    s.sweep("keep").unwrap();
    assert!(!generations.join("old.garbage.123").exists());
    assert!(generations.join("keep/generation.json").exists());
}
