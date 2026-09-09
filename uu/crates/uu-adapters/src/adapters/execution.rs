use crate::{config::Config, lanes::run_lane};
use std::time::Duration;
use uu_application::{LaneExecution, LaneExecutor};
use uu_domain::RunFacts;

use crate::runner::SystemRunner;

pub struct ConfiguredLaneExecutor<'a>(pub &'a Config);

impl LaneExecutor for ConfiguredLaneExecutor<'_> {
    fn execute(
        &self,
        name: &str,
        budget: Duration,
        deadline: Duration,
        facts: &RunFacts<'_>,
    ) -> LaneExecution {
        // One runner per lane, holding that lane's own complete budget.
        let runner = SystemRunner::for_lane(name, budget, deadline);
        match run_lane(name, self.0, facts, &runner) {
            Some(report) => LaneExecution::Reported(report),
            None => LaneExecution::Undeclared,
        }
    }
}
