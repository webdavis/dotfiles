use crate::{CommandRunner, ConfigError, LaneAdapter, RotateLogsLane};
use uu_domain::{LaneReport, RunFacts};

mod rotation;

impl LaneAdapter for RotateLogsLane {
    fn parse(label: &str, fields: toml::Table) -> Result<Self, ConfigError> {
        Self::parse_fields(label, fields)
    }

    fn keys() -> &'static [&'static str] {
        Self::KEYS
    }

    fn run(&self, name: &str, _facts: &RunFacts, runner: &dyn CommandRunner) -> LaneReport {
        let mut report = LaneReport::new(name);
        let mut under = 0;
        for log in &self.logs {
            match rotation::rotate(log.as_ref(), self, runner) {
                Ok(rotation::Observed::Rotated(size)) => {
                    report.noted(format!("rotated {log}: {size} bytes"))
                }
                Ok(rotation::Observed::UnderThreshold) => under += 1,
                Ok(rotation::Observed::Skipped) => {}
                Err(error) => report.failed(format!("{log}: {error}")),
            }
        }
        if under > 0 {
            report.noted(format!("under threshold: {under}"));
        }
        report
    }
}

#[cfg(test)]
mod tests;
