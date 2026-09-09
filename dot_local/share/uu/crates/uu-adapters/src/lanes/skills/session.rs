use super::generation::{garbage, metadata};
use super::{
    SkillsBuildMode, SkillsCandidate, SkillsEnvironment, SkillsGenerationStore, SkillsRoster,
    content,
};
use crate::{CommandRunner, SkillsConfig};
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
pub(super) struct Session<'a> {
    pub config: &'a SkillsConfig,
    pub home: &'a Path,
    pub roster: SkillsRoster,
    pub store: SkillsGenerationStore,
}
impl<'a> Session<'a> {
    pub fn open(config: &'a SkillsConfig, home: &'a Path) -> Result<Self, String> {
        let roster = SkillsRoster::read(Path::new(&config.lock))?;
        let store = SkillsGenerationStore {
            agents: Path::new(&config.agents).into(),
            roster_hash: roster.hash.clone(),
            updater_hash: capture_skills_updater()?.into(),
        };
        Ok(Self {
            config,
            home,
            roster,
            store,
        })
    }
    pub fn build(
        &self,
        mode: SkillsBuildMode,
        reinstall: &BTreeSet<String>,
        runner: &dyn CommandRunner,
    ) -> Result<SkillsCandidate, String> {
        let id = fresh_id()?;
        let candidate = self.store.candidate(&id)?;
        let build = || {
            self.seed(&candidate, mode)?;
            for name in reinstall {
                let path = candidate.agents().join("skills").join(name);
                if path.exists() {
                    std::fs::remove_dir_all(path).map_err(|e| e.to_string())?;
                }
            }
            let env = SkillsEnvironment::for_candidate(self.home, &candidate)?;
            let npx = candidate.install_npx(self.config, &self.roster, mode, &env, runner);
            let clawhub =
                candidate.install_clawhub(&self.config.clawhub, &self.roster, mode, &env, runner);
            let mut failures = Vec::new();
            for result in [npx, clawhub] {
                match result {
                    Ok(rows) => failures
                        .extend(rows.into_iter().map(|(name, why)| format!("{name}: {why}"))),
                    Err(why) => failures.push(why),
                }
            }
            if let Err(why) = candidate.assert_overlays(&self.roster) {
                failures.push(why);
            }
            if !failures.is_empty() {
                return Err(failures.join("; "));
            }
            candidate.validate(&self.roster, mode)?;
            self.store.mark_ready(
                &candidate,
                &id,
                &crate::system::iso(crate::now_epoch().ok_or("clock unavailable")?),
                mode,
            )
        };
        if let Err(why) = build() {
            let cleanup = garbage::destroy(&self.store.generations().join(&id));
            return Err(match cleanup {
                Ok(()) => why,
                Err(cleanup) => format!("{why}; discard: {cleanup}"),
            });
        }
        Ok(candidate)
    }
    fn seed(&self, candidate: &SkillsCandidate, mode: SkillsBuildMode) -> Result<(), String> {
        let source = self.store.current().join("skills");
        if source.is_dir() {
            for entry in std::fs::read_dir(source).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                let name = entry
                    .file_name()
                    .into_string()
                    .map_err(|_| "invalid generation skill name")?;
                if mode == SkillsBuildMode::Full && !self.roster.tracked_names().contains(&name) {
                    continue;
                }
                if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
                    content::copy(&entry.path(), &candidate.agents().join("skills").join(name))?;
                }
            }
        }
        candidate.absorb_store_entries(&self.store, &self.roster)?;
        let lock = self.store.current().join(".skill-lock.json");
        let value = if lock.exists() {
            metadata::document(&lock)?
        } else {
            serde_json::json!({"skills":{}})
        };
        metadata::write(&candidate.agents().join(".skill-lock.json"), &value)
    }
}
pub(super) fn fresh_id() -> Result<String, String> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    Ok(format!(
        "{}-{}-{}",
        crate::now_epoch().ok_or("clock unavailable")?,
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

pub fn capture_skills_updater() -> Result<&'static str, String> {
    static UPDATER: std::sync::OnceLock<Result<String, String>> = std::sync::OnceLock::new();
    UPDATER
        .get_or_init(|| {
            let executable = std::env::current_exe().map_err(|e| e.to_string())?;
            let bytes = std::fs::read(executable).map_err(|e| e.to_string())?;
            Ok(super::roster::digest(&bytes))
        })
        .as_ref()
        .map(String::as_str)
        .map_err(Clone::clone)
}
