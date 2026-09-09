use std::path::Path;

pub(super) const POLICY: &str = "policy:\n  allow_implicit_invocation: false\n";
pub(super) fn strip_owned(path: &Path) -> Result<Option<String>, String> {
    let original = read(path)?;
    if !original.contains(POLICY) {
        return Ok(None);
    }
    let stripped = original.replace(POLICY, "");
    if stripped.trim().is_empty() {
        std::fs::remove_file(path).map_err(|e| e.to_string())?;
    } else {
        std::fs::write(path, stripped).map_err(|e| e.to_string())?;
    }
    Ok(Some(original))
}

pub(super) fn read(path: &Path) -> Result<String, String> {
    for part in [
        path.to_path_buf(),
        path.parent().ok_or("overlay has no parent")?.to_path_buf(),
        path.parent()
            .and_then(Path::parent)
            .ok_or("overlay has no skill")?
            .to_path_buf(),
    ] {
        match std::fs::symlink_metadata(&part) {
            Ok(m) if m.file_type().is_symlink() => {
                return Err(format!("overlay symlink refused: {}", part.display()));
            }
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.to_string()),
            _ => {}
        }
    }
    match std::fs::read_to_string(path) {
        Ok(t) => Ok(t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e.to_string()),
    }
}
impl super::SkillsCandidate {
    pub fn assert_overlays(&self, roster: &super::SkillsRoster) -> Result<(), String> {
        for (name, tier) in &roster.tiers {
            let skill = self.agents().join("skills").join(name);
            if !skill.exists() {
                continue;
            }
            let path = skill.join("agents/openai.yaml");
            let content = read(&path)?;
            if tier == "on-demand" && !content.contains(POLICY) {
                reassert(&path)?;
            } else if tier == "core" {
                strip_owned(&path)?;
            }
        }
        Ok(())
    }
}
pub(super) fn reassert(path: &Path) -> Result<(), String> {
    use std::io::Write;
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
    let content = read(path)?;
    if content.contains(POLICY) {
        return Ok(());
    }
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path.parent().ok_or("overlay has no parent")?)
        .map_err(|e| e.to_string())?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(path)
        .map_err(|e| e.to_string())?;
    if !content.is_empty() && !content.ends_with('\n') {
        file.write_all(b"\n").map_err(|e| e.to_string())?;
    }
    file.write_all(POLICY.as_bytes()).map_err(|e| e.to_string())
}
#[cfg(test)]
mod tests;
