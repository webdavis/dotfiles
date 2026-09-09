use crate::{CommandRunner, Ran, SkillsConfig, Verdict};
use uu_domain::LaneReport;
impl SkillsConfig {
    pub fn refresh_app_pack(&self, runner: &dyn CommandRunner, report: &mut LaneReport) {
        if !std::path::Path::new(&self.agents)
            .join("skills/cua-driver")
            .is_symlink()
        {
            report.noted("cua-driver: no app-owned store link; skipped".into());
            return;
        }
        record(
            "cua-driver skill pack",
            runner.run_with_input(&self.cua_driver, &["skills", "update"], ""),
            report,
        );
    }
    pub fn assert_routing(&self, runner: &dyn CommandRunner, report: &mut LaneReport) {
        let checked = runner.run_with_input(&self.routing, &["--check"], "");
        if checked
            .as_ref()
            .is_ok_and(|ran| ran.verdict == Verdict::Clean)
        {
            record("superpowers routing", checked, report);
            return;
        }
        report.noted("superpowers routing drift: re-asserting the mirror".into());
        record(
            "superpowers routing",
            runner.run_with_input(&self.routing, &[], ""),
            report,
        );
    }
}
fn record(label: &str, result: Result<Ran, String>, report: &mut LaneReport) {
    match result {
        Ok(ran) => {
            let detail = format!("{label}: {}{}", ran.stdout, ran.stderr);
            match ran.verdict {
                Verdict::Clean => report.noted(detail),
                Verdict::Failed(why) | Verdict::Deferred(why) | Verdict::Pending(why) => {
                    report.failed(format!("{detail}; {why}"));
                }
            }
        }
        Err(why) => report.failed(format!("{label}: {why}")),
    }
}
#[cfg(test)]
mod tests;
