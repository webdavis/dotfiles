use crate::{CommandRunner, Config};
use uu_application::BootstrapOutcome;
use uu_domain::LaneReport;

pub trait BootstrapLane {
    fn bootstrap(&self, name: &str, home: &str, runner: &dyn CommandRunner) -> LaneReport;
}

pub fn bootstrap_lane(home: &str, config: &Config, name: &str) -> BootstrapOutcome {
    let Some(lane) = config.lanes.get(name) else {
        return BootstrapOutcome::Undeclared;
    };
    let Some(capability) = lane.adapter.bootstrap_capability() else {
        return BootstrapOutcome::Unsupported(lane.type_name().to_owned());
    };
    let runner = crate::runner::SystemRunner::for_lane(name, lane.deadline, lane.deadline);
    BootstrapOutcome::Reported(capability.bootstrap(name, home, &runner))
}

#[cfg(test)]
mod tests;
