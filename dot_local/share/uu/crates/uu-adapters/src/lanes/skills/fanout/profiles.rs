use super::SkillsRoster;
use std::collections::BTreeSet;
use std::path::Path;
pub(super) fn universe(roster: &SkillsRoster, hermes: &Path) -> Result<BTreeSet<String>, String> {
    let mut profiles = roster
        .hermes_profiles
        .values()
        .flatten()
        .cloned()
        .collect::<BTreeSet<_>>();
    if hermes.join("skills").exists() || hermes.join("skills").is_symlink() {
        profiles.insert("default".into());
    }
    let directory = hermes.join("profiles");
    if !hermes.is_symlink() && !directory.is_symlink() && directory.is_dir() {
        for entry in std::fs::read_dir(directory).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if entry.path().join("skills").exists() || entry.path().join("skills").is_symlink() {
                if let Some(name) = entry.file_name().to_str() {
                    profiles.insert(name.into());
                }
            }
        }
    }
    Ok(profiles)
}
pub(super) fn refusal(hermes: &Path, parent: &Path, skills: &Path) -> Option<String> {
    if parent.is_symlink()
        || hermes.is_symlink()
        || (parent != hermes && hermes.join("profiles").is_symlink())
    {
        return Some(format!(
            "Hermes profile parent {} is a symlink; leaving it",
            parent.display()
        ));
    }
    if skills.is_symlink() {
        return Some(format!(
            "Hermes skills dir {} is a symlink; leaving it",
            skills.display()
        ));
    }
    None
}
