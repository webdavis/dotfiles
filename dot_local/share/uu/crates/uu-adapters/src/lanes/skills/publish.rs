use super::generation::metadata::{self, Metadata};
use super::{SkillsCandidate, SkillsGenerationStore, SkillsRecovery, SkillsRoster, validate};
mod absorb;
mod store;

impl SkillsGenerationStore {
    pub fn publish(
        &self,
        candidate: &SkillsCandidate,
        roster: &SkillsRoster,
    ) -> Result<Vec<String>, String> {
        self.check_roster(roster)?;
        let mode = self.compatible(candidate)?.mode;
        candidate.validate(roster, mode)?;
        self.prepare_publish(candidate)?;
        self.complete_store_publication(roster)
    }
    pub fn recover_publish(&self, roster: &SkillsRoster) -> Result<Vec<String>, String> {
        self.check_roster(roster)?;
        if self.marker().exists() {
            let marker = metadata::document(&self.marker())?;
            let new = metadata::field(&marker, "new")?;
            if !metadata::segment(&new) {
                return Err("invalid publication identity".into());
            }
            let current = self.current();
            let source = if Metadata::read(&current).is_ok_and(|m| m.id == new) {
                current
            } else {
                self.generations().join(new).join("home/.agents")
            };
            let m = Metadata::read(&source)?;
            if m.roster_hash != self.roster_hash || m.updater_hash != self.updater_hash {
                return Err("incompatible publication metadata".into());
            }
            validate::validate_content(&source, roster, m.mode)?;
            return self.complete_store_publication(roster);
        }
        match self.recover()? {
            SkillsRecovery::Candidate(candidate) => self.publish(&candidate, roster),
            SkillsRecovery::None => Ok(Vec::new()),
            SkillsRecovery::Publication(_) => Err("publication appeared during recovery".into()),
        }
    }
    fn check_roster(&self, roster: &SkillsRoster) -> Result<(), String> {
        roster.unchanged()?;
        if roster.hash != self.roster_hash {
            return Err("publication roster identity changed".into());
        }
        Ok(())
    }
    fn complete_store_publication(&self, roster: &SkillsRoster) -> Result<Vec<String>, String> {
        let publication = match self.recover()? {
            SkillsRecovery::Publication(p) => p,
            _ => return Err("publication journal disappeared".into()),
        };
        let warnings = store::reconcile(self, roster, &publication.outgoing_names)?;
        self.finish_publication(&publication.previous_id)?;
        Ok(warnings)
    }
}
#[cfg(test)]
mod tests;
