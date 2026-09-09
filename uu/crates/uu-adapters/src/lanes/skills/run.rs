use super::generation::{garbage, metadata};
use super::{
    SkillsBuildMode, SkillsCandidate, SkillsRecovery, session::Session, snapshot::Snapshot,
};
use crate::{CommandRunner, SkillsConfig};
use std::collections::BTreeSet;
use std::path::Path;
use uu_domain::LaneReport;
impl SkillsConfig {
    pub fn run_skills(&self, name: &str, home: &Path, runner: &dyn CommandRunner) -> LaneReport {
        let mut report = LaneReport::new(name);
        let session = match Session::open(self, home) {
            Ok(session) => session,
            Err(why) => {
                report.failed(why);
                return report;
            }
        };
        let mut recovered = session.recover();
        if recovered.is_ok() {
            report.noted("recovery completed".into());
        }
        if recovered.is_ok() && session.migration_needed() {
            match session.migrate() {
                Ok(()) => report.noted("flat-store migration completed".into()),
                Err(why) => {
                    report.failed(format!("flat-store migration: {why}"));
                    recovered = Err(why);
                }
            }
            if recovered.is_ok() {
                recovered = session.recover();
                if recovered.is_ok() {
                    report.noted("recovery completed after migration".into());
                }
            }
        }
        let before = Snapshot::read(&session.store.agents);
        match recovered {
            Ok(candidate) => {
                if let Err(why) = session.weekly_candidate(candidate, runner, &mut report) {
                    report.failed(format!("weekly candidate: {why}"));
                }
            }
            Err(why) => report.failed(format!("recovery: {why}; candidate build withheld")),
        }
        self.refresh_app_pack(runner, &mut report);
        session.deliver(SkillsBuildMode::Full, &mut report);
        session.verify_overlays(&mut report);
        self.assert_routing(runner, &mut report);
        session
            .roster
            .update_hermes_registry(self, runner, &mut report);
        session
            .roster
            .forks
            .report(&std::env::temp_dir(), runner, &mut report);
        before.report(&Snapshot::read(&session.store.agents), &mut report);
        if let Err(why) = session.roster.unchanged() {
            report.failed(why);
        }
        report
    }
}
impl Session<'_> {
    fn recover(&self) -> Result<Option<SkillsCandidate>, String> {
        let recovery = || {
            if self.store.marker().exists() {
                self.store.recover_publish(&self.roster)?;
            }
            let reusable = match self.store.recover()? {
                SkillsRecovery::Candidate(c) => Some(c),
                _ => None,
            };
            if self.store.generations().is_dir() {
                for entry in
                    std::fs::read_dir(self.store.generations()).map_err(|e| e.to_string())?
                {
                    let path = entry.map_err(|e| e.to_string())?.path();
                    if (path.join("home").exists() || path.join("discarded-home").exists())
                        && reusable
                            .as_ref()
                            .is_none_or(|c| c.home.parent() != Some(path.as_path()))
                    {
                        garbage::destroy(&path)?;
                    }
                }
            }
            Ok::<_, String>(reusable)
        };
        recovery()
    }
    fn weekly_candidate(
        &self,
        recovered: Option<SkillsCandidate>,
        runner: &dyn CommandRunner,
        report: &mut LaneReport,
    ) -> Result<(), String> {
        if let Some(candidate) = recovered {
            report.noted("reusing recovered candidate".into());
            for line in self.store.publish(&candidate, &self.roster)? {
                report.noted(line);
            }
        } else {
            let candidate = self.build(SkillsBuildMode::Full, &BTreeSet::new(), runner)?;
            for line in self.store.publish(&candidate, &self.roster)? {
                report.noted(line);
            }
        }
        let metadata = metadata::document(&self.store.current().join("generation.json"))?;
        report.noted(format!(
            "generation published: {}",
            metadata::field(&metadata, "id")?
        ));
        Ok(())
    }
    pub(super) fn deliver(&self, mode: SkillsBuildMode, report: &mut LaneReport) {
        match self.store.fanout(
            &self.roster,
            Path::new(&self.config.claude_skills),
            Path::new(&self.config.hermes),
            mode,
        ) {
            Ok(warnings) => {
                for line in warnings {
                    report.noted(line);
                }
                report.noted("skills fan-out completed".into());
            }
            Err(why) => report.failed(format!("skills fan-out: {why}")),
        }
    }
    pub(super) fn verify_overlays(&self, report: &mut LaneReport) {
        for (name, tier) in &self.roster.tiers {
            if tier != "on-demand" {
                continue;
            }
            let skill = self.store.agents.join("skills").join(name);
            let result = if skill.is_symlink() {
                let target = Path::new("../.skills-current/skills").join(name);
                if std::fs::read_link(&skill).ok().as_deref() != Some(target.as_path()) {
                    continue;
                }
                let path = self
                    .store
                    .current()
                    .join("skills")
                    .join(name)
                    .join("agents/openai.yaml");
                super::overlay::read(&path).and_then(|content| {
                    if content.contains(super::overlay::POLICY) {
                        Ok(())
                    } else {
                        Err("live overlay is missing; next candidate must repair it".into())
                    }
                })
            } else if skill.is_dir() {
                super::overlay::reassert(&skill.join("agents/openai.yaml"))
            } else {
                continue;
            };
            if let Err(why) = result {
                report.failed(format!("live overlay {name}: {why}"));
            }
        }
        report.noted("live overlays checked".into());
    }
}
#[cfg(test)]
mod tests;
