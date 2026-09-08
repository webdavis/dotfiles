use super::super::{SkillsBuildMode, SkillsGenerationStore, SkillsRoster};
use super::{Metadata, absorb, metadata};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
fn target(name: &str) -> PathBuf {
    PathBuf::from(format!("../.skills-current/skills/{name}"))
}
fn quarantine(store: &SkillsGenerationStore, path: &Path, id: &str) -> Result<(), String> {
    let parent = store.agents.join(".skills-quarantine").join(id);
    metadata::directory(&parent)?;
    let to = parent.join(path.file_name().ok_or("store entry has no name")?);
    if to.exists() || to.is_symlink() {
        return Err("store quarantine already contains this name".into());
    }
    std::fs::rename(path, to).map_err(|e| format!("store quarantine: {e}"))
}
fn plant(path: &Path, target: &Path) -> Result<(), String> {
    if path.is_symlink() {
        std::fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    std::os::unix::fs::symlink(target, path).map_err(|e| e.to_string())
}
pub(super) fn reconcile(
    store: &SkillsGenerationStore,
    roster: &SkillsRoster,
    outgoing: &BTreeSet<String>,
) -> Result<Vec<String>, String> {
    let current = store.current();
    let m = Metadata::read(&current)?;
    let directory = store.agents.join("skills");
    metadata::directory(&directory)?;
    let recorded = current.join(".reabsorbed.json");
    let absorbed = if recorded.exists() {
        metadata::document(&recorded)?
    } else {
        serde_json::json!({})
    };
    let mut warnings = Vec::new();
    for name in roster.tracked_names() {
        let path = directory.join(&name);
        if !path.exists() && !path.is_symlink() {
            plant(&path, &target(&name))?;
            continue;
        }
        if m.mode == SkillsBuildMode::Additive {
            continue;
        }
        if std::fs::symlink_metadata(&path).is_ok_and(|v| v.is_dir()) {
            if let Some(expected) = absorbed.get(&name).and_then(|v| v.as_str()) {
                if absorb::fingerprint(&path)? == expected {
                    quarantine(store, &path, &m.id)?;
                    plant(&path, &target(&name))?;
                } else {
                    warnings.push(format!("store/{name} changed after absorption; leaving it"));
                }
            } else {
                warnings.push(format!(
                    "store/{name} is an unseen competing writer; leaving it"
                ));
            }
        } else if path.is_symlink()
            && std::fs::read_link(&path).map_err(|e| e.to_string())? != target(&name)
        {
            let existing = std::fs::read_link(&path).map_err(|e| e.to_string())?;
            if existing.starts_with("../.skills-current") {
                plant(&path, &target(&name))?;
            } else {
                warnings.push(format!("store/{name} is a foreign symlink; leaving it"));
            }
        }
    }
    if m.mode == SkillsBuildMode::Full {
        prune(store, roster, outgoing, &m.id)?;
    }
    let lock = store.agents.join(".skill-lock.json");
    let desired = Path::new(".skills-current/.skill-lock.json");
    if !lock.exists() && !lock.is_symlink() {
        plant(&lock, desired)?;
    } else if m.mode == SkillsBuildMode::Full
        && std::fs::read_link(&lock).ok().as_deref() != Some(desired)
    {
        if !lock.is_symlink() {
            quarantine(store, &lock, &m.id)?;
        }
        plant(&lock, desired)?;
    }
    Ok(warnings)
}
fn prune(
    store: &SkillsGenerationStore,
    roster: &SkillsRoster,
    outgoing: &BTreeSet<String>,
    id: &str,
) -> Result<(), String> {
    let tracked = roster.tracked_names();
    for entry in std::fs::read_dir(store.agents.join("skills")).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "non-UTF-8 store name")?;
        if tracked.contains(&name) {
            continue;
        }
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        if kind.is_dir() && outgoing.contains(&name) {
            quarantine(store, &entry.path(), id)?;
        } else if kind.is_symlink()
            && std::fs::read_link(entry.path()).map_err(|e| e.to_string())? == target(&name)
        {
            std::fs::remove_file(entry.path()).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
