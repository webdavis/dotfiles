use super::super::{SkillsBuildMode, SkillsCandidate, SkillsGenerationStore, SkillsRoster};
use crate::lanes::skills::tests::directory;
use serde_json::json;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
fn skill(path: &Path, content: &str) {
    fs::create_dir_all(path).unwrap();
    fs::write(path.join("SKILL.md"), content).unwrap();
}
fn fixture() -> (PathBuf, SkillsGenerationStore, SkillsRoster) {
    let root = directory();
    let lock = root.join("roster.json");
    fs::write(
        &lock,
        json!({"version":2,"npxTracked":{"alpha":{"repo":"owner/repo"}}}).to_string(),
    )
    .unwrap();
    let roster = SkillsRoster::read(&lock).unwrap();
    fs::write(root.join("uu"), "code-v1").unwrap();
    let store =
        SkillsGenerationStore::capture(&root.join(".agents"), &roster.hash, &root.join("uu"))
            .unwrap();
    fs::create_dir_all(store.agents.join("skills")).unwrap();
    (root, store, roster)
}
fn ready(store: &SkillsGenerationStore, id: &str, mode: SkillsBuildMode) -> SkillsCandidate {
    let c = store.candidate(id).unwrap();
    skill(&c.agents().join("skills/alpha"), id);
    fs::write(
        c.agents().join(".skill-lock.json"),
        r#"{"skills":{"alpha":{}}}"#,
    )
    .unwrap();
    store
        .mark_ready(&c, id, "2026-09-07T00:00:00Z", mode)
        .unwrap();
    c
}
fn live(store: &SkillsGenerationStore) {
    let c = ready(store, "old", SkillsBuildMode::Full);
    skill(&c.agents().join("skills/delisted"), "outgoing");
    fs::rename(c.agents(), store.agents.join(".skills-current")).unwrap();
}
fn target(name: &str) -> PathBuf {
    PathBuf::from(format!("../.skills-current/skills/{name}"))
}
mod links;
mod pruning;
mod recovery;
