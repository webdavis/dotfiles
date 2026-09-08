use super::super::{SkillsBuildMode, SkillsGenerationStore, SkillsRoster};
use crate::lanes::skills::tests::directory;
use serde_json::json;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
fn skill(path: &Path) {
    fs::create_dir_all(path).unwrap();
    fs::write(path.join("SKILL.md"), "content").unwrap();
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
    skill(&store.agents.join("skills/alpha"));
    (root, store, roster)
}
fn fanout(
    root: &Path,
    store: &SkillsGenerationStore,
    roster: &SkillsRoster,
    mode: SkillsBuildMode,
) -> Vec<String> {
    store
        .fanout(
            roster,
            &root.join(".claude/skills"),
            &root.join(".hermes"),
            mode,
        )
        .unwrap()
}
fn delivered(path: &Path, expected: &str) {
    assert_eq!(fs::read_link(path).ok(), Some(PathBuf::from(expected)));
    assert_eq!(
        fs::read_to_string(path.join("SKILL.md")).unwrap_or_default(),
        "content"
    );
}
fn mode_values() -> [SkillsBuildMode; 2] {
    [SkillsBuildMode::Full, SkillsBuildMode::Additive]
}
mod convergence;
mod eligibility;
mod guards;
