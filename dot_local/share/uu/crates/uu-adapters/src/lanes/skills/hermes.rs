use super::SkillsRoster;
use crate::{CommandRunner, SkillsConfig};
use std::collections::BTreeSet;
use uu_domain::LaneReport;
impl SkillsRoster {
    pub fn update_hermes_registry(
        &self,
        config: &SkillsConfig,
        runner: &dyn CommandRunner,
        report: &mut LaneReport,
    ) {
        let profiles: BTreeSet<_> = self
            .hermes_registry
            .values()
            .flat_map(|entry| &entry.profiles)
            .collect();
        for profile in profiles {
            for (name, entry) in &self.hermes_registry {
                if !entry.profiles.contains(profile) {
                    continue;
                }
                if entry.held {
                    report.noted(format!("hermes {profile}/{name}: held, skipped"));
                    continue;
                }
                let label = format!("hermes {profile}/{}", entry.lock_key);
                let result = runner.run(
                    &config.hermes_cli,
                    &["-p", profile, "skills", "update", &entry.lock_key],
                );
                match result {
                    Ok(output) => {
                        let lowered = output.to_lowercase();
                        if lowered.contains("blocked") || lowered.contains("refused") {
                            report.failed(format!("{label}: blocked/refused: {output}"));
                        } else {
                            report.noted(format!("{label}: updated: {output}"));
                        }
                    }
                    Err(why) => report.failed(format!("{label}: {why}")),
                }
            }
        }
    }
}
#[cfg(test)]
mod tests;
