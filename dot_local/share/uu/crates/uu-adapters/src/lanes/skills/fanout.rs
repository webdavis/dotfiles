use super::{SkillsBuildMode, SkillsGenerationStore, SkillsRoster};
use std::collections::BTreeSet;
use std::path::Path;
mod converge;
mod profiles;

impl SkillsGenerationStore {
    pub fn fanout(
        &self,
        roster: &SkillsRoster,
        claude: &Path,
        hermes: &Path,
        mode: SkillsBuildMode,
    ) -> Result<Vec<String>, String> {
        let store = self.agents.join("skills");
        let surviving = std::fs::read_dir(&store)
            .map_err(|e| e.to_string())?
            .map(|e| {
                let e = e.map_err(|e| e.to_string())?;
                let kind = e.file_type().map_err(|e| e.to_string())?;
                Ok(if kind.is_dir() || kind.is_symlink() {
                    e.file_name().into_string().ok()
                } else {
                    None
                })
            })
            .collect::<Result<Vec<_>, String>>()?
            .into_iter()
            .flatten()
            .collect::<BTreeSet<_>>();
        let claude_names = surviving
            .difference(&roster.claude_undelivered)
            .cloned()
            .collect();
        let mut warnings = Vec::new();
        if claude.is_symlink() {
            warnings.push(format!(
                "Claude skills dir {} is a symlink; leaving it",
                claude.display()
            ));
        } else {
            warnings.extend(converge::directory(
                claude,
                "../../.agents/skills",
                &store,
                &claude_names,
                mode,
            )?);
        }
        for profile in profiles::universe(roster, hermes)? {
            let parent = if profile == "default" {
                hermes.to_path_buf()
            } else {
                hermes.join("profiles").join(&profile)
            };
            let destination = parent.join("skills");
            if let Some(why) = profiles::refusal(hermes, &parent, &destination) {
                warnings.push(why);
                continue;
            }
            let desired = surviving
                .iter()
                .filter(|name| {
                    !matches!(name.as_str(), "humanizer" | "hyperframes")
                        && roster
                            .hermes_profiles
                            .get(*name)
                            .is_some_and(|p| p.contains(&profile))
                })
                .cloned()
                .collect();
            let prefix = if profile == "default" {
                "../../.agents/skills"
            } else {
                "../../../../.agents/skills"
            };
            warnings.extend(converge::directory(
                &destination,
                prefix,
                &store,
                &desired,
                mode,
            )?);
        }
        Ok(warnings)
    }
}
#[cfg(test)]
mod tests;
