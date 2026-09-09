use super::generation::{
    garbage,
    metadata::{self, Metadata},
};
use super::{
    SkillsBuildMode, content, exchange_skills_directories,
    session::{Session, fresh_id},
};
use std::path::Path;
impl Session<'_> {
    pub fn migration_needed(&self) -> bool {
        self.store.agents.join("skills").is_dir()
            && !(Metadata::read(&self.store.current()).is_ok()
                && self.store.current().join("skills").is_dir()
                && self.store.current().join(".skill-lock.json").is_file())
    }
    pub fn migrate(&self) -> Result<(), String> {
        let id = fresh_id()?;
        let candidate = self.store.candidate(&id)?;
        for name in self.roster.tracked_names() {
            let source = self.store.agents.join("skills").join(&name);
            if std::fs::symlink_metadata(&source).is_ok_and(|m| m.is_dir()) {
                content::copy(&source, &candidate.agents().join("skills").join(name))?;
            }
        }
        let flat = self.store.agents.join(".skill-lock.json");
        if flat.is_file() && !flat.is_symlink() {
            std::fs::copy(&flat, candidate.agents().join(".skill-lock.json"))
                .map_err(|e| e.to_string())?;
        } else {
            metadata::write(
                &candidate.agents().join(".skill-lock.json"),
                &serde_json::json!({"skills":{}}),
            )?;
        }
        self.store.mark_ready(
            &candidate,
            &id,
            &crate::system::iso(crate::now_epoch().ok_or("clock unavailable")?),
            SkillsBuildMode::Full,
        )?;
        let current = self.store.current();
        if current.is_symlink() {
            return Err("migration refuses a symlinked current generation".into());
        }
        if current.exists() {
            exchange_skills_directories(&candidate.agents(), &current)?;
        } else {
            std::fs::rename(candidate.agents(), &current).map_err(|e| e.to_string())?;
        }
        for name in self.roster.tracked_names() {
            if !current.join("skills").join(&name).is_dir() {
                continue;
            }
            let path = self.store.agents.join("skills").join(&name);
            replace_flat(
                &path,
                Path::new(&format!("../.skills-current/skills/{name}")),
                &id,
            )?;
        }
        replace_flat(&flat, Path::new(".skills-current/.skill-lock.json"), &id)?;
        garbage::destroy(&self.store.generations().join(id))
    }
}
fn replace_flat(path: &Path, target: &Path, id: &str) -> Result<(), String> {
    if path.is_symlink() {
        return Ok(());
    }
    if !path.exists() {
        return std::os::unix::fs::symlink(target, path).map_err(|e| e.to_string());
    }
    let name = path
        .file_name()
        .ok_or("flat entry has no name")?
        .to_string_lossy();
    let staged = path.with_file_name(format!(".{name}.migrating.{id}"));
    std::os::unix::fs::symlink(target, &staged).map_err(|e| e.to_string())?;
    exchange_skills_directories(&staged, path)?;
    if staged.is_dir() {
        garbage::destroy(&staged)
    } else {
        std::fs::remove_file(&staged).map_err(|e| e.to_string())
    }
}
