use super::{SkillsBuildMode, session::Session};
use crate::{CommandRunner, SkillsConfig};
use std::collections::BTreeSet;
use std::path::Path;
use uu_domain::LaneReport;
mod health;
impl SkillsConfig {
    pub fn bootstrap_skills(
        &self,
        name: &str,
        home: &Path,
        runner: &dyn CommandRunner,
    ) -> LaneReport {
        let mut report = LaneReport::new(name);
        let session = match Session::open(self, home) {
            Ok(session) => session,
            Err(why) => {
                report.failed(why);
                return report;
            }
        };
        if let Err(why) = session.bootstrap_candidate(runner, &mut report) {
            report.failed(format!("bootstrap candidate: {why}"));
        }
        session.deliver(SkillsBuildMode::Additive, &mut report);
        session.verify_overlays(&mut report);
        self.assert_routing(runner, &mut report);
        if let Err(why) = session.roster.unchanged() {
            report.failed(why);
        }
        report
    }
}
impl Session<'_> {
    fn bootstrap_candidate(
        &self,
        runner: &dyn CommandRunner,
        report: &mut LaneReport,
    ) -> Result<(), String> {
        let mut repair = false;
        let mut reinstall = BTreeSet::new();
        for name in self.roster.tracked_names() {
            if let Some(reason) = self.health(&name) {
                repair = true;
                if reason != "overlay" {
                    reinstall.insert(name.clone());
                }
                report.noted(format!("bootstrap: {name} needs repair ({reason})"));
            }
        }
        if !repair {
            report.noted("bootstrap: every roster skill is healthy; no publication".into());
            return Ok(());
        }
        let candidate = self.build(SkillsBuildMode::Additive, &reinstall, runner)?;
        for line in self.store.publish(&candidate, &self.roster)? {
            report.noted(line);
        }
        report.noted("bootstrap generation published".into());
        Ok(())
    }
}
#[cfg(test)]
mod tests;
