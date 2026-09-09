use crate::{LockFailure, RunState};
use uu_domain::LaneReport;

#[derive(Debug, PartialEq, Eq)]
pub enum BootstrapOutcome {
    Reported(LaneReport),
    Undeclared,
    Unsupported(String),
    LockRefused(LockFailure),
}

pub fn bootstrap(
    state: &impl RunState,
    perform: impl FnOnce() -> BootstrapOutcome,
) -> BootstrapOutcome {
    let _guard = match state.acquire() {
        Ok(guard) => guard,
        Err(error) => return BootstrapOutcome::LockRefused(error),
    };
    perform()
}

#[cfg(test)]
mod tests;
