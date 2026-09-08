use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
mod cleanup;
mod exchange;
mod garbage;
pub(super) mod metadata;
mod recovery;
pub use exchange::exchange_skills_directories;
use metadata::{Metadata, directory, segment};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillsBuildMode {
    Full,
    Additive,
}
#[derive(Debug, Clone)]
pub struct SkillsCandidate {
    pub home: PathBuf,
}
impl SkillsCandidate {
    pub fn agents(&self) -> PathBuf {
        self.home.join(".agents")
    }
    fn id(&self) -> Result<String, String> {
        self.home
            .parent()
            .and_then(Path::file_name)
            .and_then(|s| s.to_str())
            .filter(|s| segment(s))
            .map(str::to_owned)
            .ok_or_else(|| "invalid candidate directory id".into())
    }
}
#[derive(Debug)]
pub struct SkillsPublication {
    pub outgoing_names: BTreeSet<String>,
    pub previous_id: String,
}
#[derive(Debug)]
pub enum SkillsRecovery {
    None,
    Candidate(SkillsCandidate),
    Publication(SkillsPublication),
}
pub struct SkillsGenerationStore {
    pub agents: PathBuf,
    pub updater_hash: String,
    pub roster_hash: String,
}
impl SkillsGenerationStore {
    pub fn capture(agents: &Path, roster_hash: &str, executable: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(executable)
            .map_err(|e| format!("updater {}: {e}", executable.display()))?;
        Ok(Self {
            agents: agents.into(),
            updater_hash: super::roster::digest(&bytes),
            roster_hash: roster_hash.into(),
        })
    }
    pub fn candidate(&self, id: &str) -> Result<SkillsCandidate, String> {
        if !segment(id) {
            return Err("invalid candidate id".into());
        }
        let home = self.generations().join(id).join("home");
        if home.exists() {
            return Err(format!("candidate {id} already exists"));
        }
        directory(&home.join(".agents/skills"))?;
        Ok(SkillsCandidate { home })
    }
    pub fn mark_ready(
        &self,
        candidate: &SkillsCandidate,
        id: &str,
        created_at: &str,
        mode: SkillsBuildMode,
    ) -> Result<(), String> {
        if candidate.id()? != id {
            return Err("metadata id disagrees with candidate directory".into());
        }
        Metadata {
            id: id.into(),
            roster_hash: self.roster_hash.clone(),
            updater_hash: self.updater_hash.clone(),
            mode,
        }
        .write(&candidate.agents(), created_at)
    }
    pub(super) fn compatible(&self, candidate: &SkillsCandidate) -> Result<Metadata, String> {
        let m = Metadata::read(&candidate.agents())?;
        if m.id != candidate.id()?
            || m.roster_hash != self.roster_hash
            || m.updater_hash != self.updater_hash
        {
            return Err("incompatible candidate metadata".into());
        }
        Ok(m)
    }
    pub(super) fn generations(&self) -> PathBuf {
        self.agents.join(".skills-generations")
    }
    pub(super) fn current(&self) -> PathBuf {
        self.agents.join(".skills-current")
    }
    pub(super) fn marker(&self) -> PathBuf {
        self.agents.join(".skills-exchange.json")
    }
    pub fn prepare_publish(&self, candidate: &SkillsCandidate) -> Result<(), String> {
        let new = self.compatible(candidate)?;
        let old = match std::fs::symlink_metadata(self.current()) {
            Ok(m) if m.is_dir() => Metadata::read(&self.current())?.id,
            Ok(_) => return Err("current generation is not a real directory".into()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e.to_string()),
        };
        if self.marker().exists() {
            return Err("publication already in flight".into());
        }
        let outgoing = if old.is_empty() {
            BTreeSet::new()
        } else {
            std::fs::read_dir(self.current().join("skills"))
                .map_err(|e| e.to_string())?
                .map(|e| {
                    e.map(|e| e.file_name().to_string_lossy().into_owned())
                        .map_err(|e| e.to_string())
                })
                .collect::<Result<BTreeSet<_>, _>>()?
        };
        metadata::write(
            &self.marker(),
            &serde_json::json!({"new":new.id,"old":old,"outgoing":outgoing}),
        )
    }
    pub fn recover(&self) -> Result<SkillsRecovery, String> {
        if self.marker().exists() {
            return self.recover_publication().map(SkillsRecovery::Publication);
        }
        if !self.generations().exists() {
            return Ok(SkillsRecovery::None);
        }
        let mut entries = std::fs::read_dir(self.generations())
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let candidate = SkillsCandidate {
                home: entry.path().join("home"),
            };
            if let Ok(m) = self.compatible(&candidate) {
                if m.mode == SkillsBuildMode::Full {
                    return Ok(SkillsRecovery::Candidate(candidate));
                }
            }
        }
        Ok(SkillsRecovery::None)
    }
}
#[cfg(test)]
mod tests;
