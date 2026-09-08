use super::{SkillsBuildMode, SkillsCandidate, SkillsRoster};
use crate::{CommandRunner, SkillsConfig};
use std::collections::BTreeMap;
use std::os::unix::fs::DirBuilderExt;
use std::path::Path;
mod lock;

pub struct SkillsEnvironment {
    pub variables: BTreeMap<String, String>,
}
impl SkillsEnvironment {
    pub fn for_candidate(real_home: &Path, candidate: &SkillsCandidate) -> Result<Self, String> {
        let interpreter_path = format!(
            "{}:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin",
            real_home
                .join(".local/share/fnm/aliases/default/bin")
                .display()
        );
        let mut variables = BTreeMap::new();
        for (key, relative) in [
            ("HOME", ""),
            ("XDG_CACHE_HOME", ".cache"),
            ("XDG_CONFIG_HOME", ".config"),
            ("XDG_DATA_HOME", ".local/share"),
            ("XDG_STATE_HOME", ".local/state"),
            ("TMPDIR", ".tmp"),
            ("npm_config_cache", ".npm"),
        ] {
            let path = if relative.is_empty() {
                candidate.home.clone()
            } else {
                candidate.home.join(relative)
            };
            std::fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(&path)
                .map_err(|e| e.to_string())?;
            variables.insert(key.into(), path.to_string_lossy().into_owned());
        }
        let clawhub_config = candidate.home.join(".config/clawhub");
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&clawhub_config)
            .map_err(|e| e.to_string())?;
        variables.insert(
            "CLAWHUB_CONFIG_PATH".into(),
            clawhub_config
                .join("config.json")
                .to_string_lossy()
                .into_owned(),
        );
        variables.insert("PATH".into(), interpreter_path);
        Ok(Self { variables })
    }
}
impl SkillsCandidate {
    pub fn install_npx(
        &self,
        config: &SkillsConfig,
        roster: &SkillsRoster,
        mode: SkillsBuildMode,
        env: &SkillsEnvironment,
        runner: &dyn CommandRunner,
    ) -> Result<BTreeMap<String, String>, String> {
        let mut groups: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for (name, repo) in &roster.npx {
            if mode == SkillsBuildMode::Additive && self.agents().join("skills").join(name).exists()
            {
                continue;
            }
            groups.entry(repo).or_default().push(name);
        }
        let mut failed = BTreeMap::new();
        for (repo, names) in groups {
            let version = format!("skills@{}", config.skills_cli_version);
            let mut args = vec!["--yes", &version, "add", repo];
            for name in &names {
                args.extend(["--skill", name]);
            }
            args.extend(["--agent", "claude-code", "--agent", "codex", "-g", "-y"]);
            if let Err(why) = runner.run_in(&config.npx, &args, &env.variables) {
                for name in names {
                    failed.insert(name.to_string(), why.clone());
                }
            }
        }
        lock::reconcile(self, roster, mode)?;
        Ok(failed)
    }
}
#[cfg(test)]
mod tests;
