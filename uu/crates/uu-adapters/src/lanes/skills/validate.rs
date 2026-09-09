use super::{SkillsBuildMode, SkillsCandidate, SkillsRoster, overlay};
use std::collections::BTreeSet;
use std::path::Path;

fn regular(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|m| m.is_file())
}
impl SkillsCandidate {
    pub fn validate(&self, roster: &SkillsRoster, mode: SkillsBuildMode) -> Result<(), String> {
        match validate_content(&self.agents(), roster, mode) {
            Ok(()) => Ok(()),
            Err(why) => {
                let discarded = self.home.with_file_name("discarded-home");
                std::fs::rename(&self.home, &discarded)
                    .map_err(|e| format!("{why}; candidate discard refused: {e}"))?;
                Err(format!(
                    "{why}; candidate discarded at {}",
                    discarded.display()
                ))
            }
        }
    }
}
pub(super) fn validate_content(
    agents: &Path,
    roster: &SkillsRoster,
    mode: SkillsBuildMode,
) -> Result<(), String> {
    let lock = agents.join(".skill-lock.json");
    if !regular(&lock) {
        return Err("candidate lock is missing or not a regular file".into());
    }
    let bytes = std::fs::read(&lock).map_err(|e| format!("candidate lock: {e}"))?;
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|e| format!("candidate lock: {e}"))?;
    let rows = value
        .get("skills")
        .and_then(|v| v.as_object())
        .ok_or("candidate lock has no skills object")?;
    if mode == SkillsBuildMode::Full
        && rows.keys().cloned().collect::<BTreeSet<_>>()
            != roster.npx.keys().cloned().collect::<BTreeSet<_>>()
    {
        return Err("candidate lock keys do not equal npxTracked".into());
    }
    for name in roster.tracked_names() {
        let skill = agents.join("skills").join(&name);
        if !std::fs::symlink_metadata(&skill).is_ok_and(|m| m.is_dir())
            || !regular(&skill.join("SKILL.md"))
        {
            return Err(format!(
                "tracked skill `{name}` is missing its directory or SKILL.md"
            ));
        }
        if roster.clawhub.contains_key(&name) && !regular(&skill.join(".clawhub/origin.json")) {
            return Err(format!("clawhub skill `{name}` has no origin.json"));
        }
    }
    for (name, tier) in &roster.tiers {
        let skill = agents.join("skills").join(name);
        if !skill.exists() {
            continue;
        }
        let content = overlay::read(&skill.join("agents/openai.yaml"))?;
        if content.contains(overlay::POLICY) != (tier == "on-demand") {
            return Err(format!("skill `{name}` has a drifted Codex overlay"));
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests;
