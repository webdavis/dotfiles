use std::time::Duration;
use uu_domain::{LaneReport, RunFacts};

#[derive(Debug, PartialEq, Eq)]
pub enum ClockFailure {
    BeforeEpoch,
}

pub trait RunClock {
    type Tick;

    fn epoch(&self) -> Result<i64, ClockFailure>;
    fn start(&self) -> Self::Tick;
    fn elapsed(&self, tick: &Self::Tick) -> Duration;
}

pub enum LaneExecution {
    Reported(LaneReport),
    Undeclared,
}

pub trait LaneExecutor {
    fn execute(
        &self,
        name: &str,
        budget: Duration,
        deadline: Duration,
        facts: &RunFacts<'_>,
    ) -> LaneExecution;
}
