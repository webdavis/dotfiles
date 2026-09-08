use super::Session;
use crate::lanes::skills::{generation::metadata, overlay};
use std::path::Path;
impl Session<'_> {
    pub(super) fn health(&self, name: &str) -> Option<&'static str> {
        let skill = self.store.agents.join("skills").join(name);
        if !skill.exists() && !skill.is_symlink() {
            return Some("absent");
        }
        if self.migration_needed() {
            return if skill.join("SKILL.md").is_file() {
                None
            } else {
                Some("skillmd")
            };
        }
        let target = Path::new("../.skills-current/skills").join(name);
        if std::fs::read_link(&skill).ok().as_deref() != Some(target.as_path())
            || !self.store.current().join("skills").join(name).is_dir()
        {
            return Some("link");
        }
        if !skill.join("SKILL.md").is_file() {
            return Some("skillmd");
        }
        if self.roster.npx.contains_key(name) {
            let lock = metadata::document(&self.store.current().join(".skill-lock.json"));
            if lock
                .ok()
                .and_then(|v| v.get("skills").and_then(|s| s.get(name)).cloned())
                .is_none()
            {
                return Some("lock");
            }
        }
        let on_demand = self
            .roster
            .tiers
            .get(name)
            .is_some_and(|tier| tier == "on-demand");
        let content = overlay::read(
            &self
                .store
                .current()
                .join("skills")
                .join(name)
                .join("agents/openai.yaml"),
        )
        .unwrap_or_default();
        if content.contains(overlay::POLICY) != on_demand {
            return Some("overlay");
        }
        None
    }
}
