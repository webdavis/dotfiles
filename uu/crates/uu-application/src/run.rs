//! Run ordering owns the lock, header sampling, lane budgets and marker advancement.

use std::collections::BTreeMap;
use std::time::Duration;
use uu_domain::{LaneReport, RunFacts, lane_budget};

use crate::delivery::{deliver_record, send_alert};
use crate::ports::{
    AlarmKind, AlertTarget, LaneExecution, LaneExecutor, LockFailure, Notice, RunClock,
    RunDelivery, RunPresentation, RunRecord, RunState,
};
use crate::staleness::track_staleness;

pub struct LaneSettings {
    pub deadline: Duration,
    pub escalate_after_runs: std::num::NonZeroU32,
}

pub struct RunRequest<'a> {
    pub lanes: &'a BTreeMap<String, LaneSettings>,
    pub only: Option<&'a str>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum RunOutcome {
    Completed,
    UndeclaredLane,
    LockRefused(LockFailure),
}

pub struct Run<S, C, L, D, P> {
    pub state: S,
    pub clock: C,
    pub lanes: L,
    pub delivery: D,
    pub presentation: P,
}

impl<S: RunState, C: RunClock, L: LaneExecutor, D: RunDelivery, P: RunPresentation>
    Run<S, C, L, D, P>
{
    pub fn execute(&self, request: RunRequest<'_>) -> RunOutcome {
        let _lock = match self.state.acquire() {
            Ok(lock) => lock,
            Err(failure) => return RunOutcome::LockRefused(failure),
        };

        // Prune under the same lock. A new lane reusing a removed name must not
        // inherit its streak and alert on its very first miss.
        let declared: Vec<&str> = request.lanes.keys().map(String::as_str).collect();
        self.state.prune_removed_lanes(&declared);

        // Sample the header before work or marker publication. A clock that
        // cannot be read renders as epoch zero, never a plausible current date.
        let started = self.clock.epoch().unwrap_or(0);
        let marker = self.state.marker();
        let header = self.presentation.header(started, &marker);
        let facts = RunFacts {
            host: &header.host,
            started_epoch: started,
            started_iso: &header.started_iso,
            marker: &marker.value,
        };
        let reports = self.run_lanes(&request, &facts);
        if request.only.is_some() && reports.is_empty() {
            return RunOutcome::UndeclaredLane;
        }

        let failures = reports.iter().map(|report| report.failures()).sum();
        let deferred = reports
            .iter()
            .filter(|report| report.verdict() == uu_domain::LaneVerdict::Deferred)
            .count();
        let pending = reports
            .iter()
            .filter(|report| report.verdict() == uu_domain::LaneVerdict::Pending)
            .count();
        let detail = self.presentation.write_record(&header, &reports);
        for report in reports.iter().filter(|report| report.failures() > 0) {
            send_alert(
                &self.delivery,
                &self.presentation,
                AlarmKind::Failed,
                &header.host,
                AlertTarget::Lane(&report.name),
                &uu_domain::alert_summary(report),
            );
        }
        track_staleness(
            &self.state,
            &self.delivery,
            &self.presentation,
            &header.host,
            &reports,
        );
        crate::pending::track_pending(
            &self.state,
            &self.delivery,
            &self.presentation,
            &header.host,
            request.lanes,
            &reports,
        );
        let record = RunRecord {
            host: &header.host,
            failures,
            deferred,
            pending,
            detail: &detail,
        };
        let record_lost = !deliver_record(&self.delivery, &self.presentation, record);

        // An unreceived record and a deferred lane both leave the old marker.
        // Read the finish clock here: using the header would add this run's own
        // duration to every following gap. An unreadable clock must overstate
        // the next gap rather than silently understating it with a guessed time.
        if failures == 0 && deferred == 0 && !record_lost {
            match self.clock.epoch() {
                Ok(finished) => {
                    if let Err(failure) = self.state.write_marker(finished) {
                        self.presentation
                            .notice(Notice::MarkerWriteFailed(&failure));
                    }
                }
                Err(_) => self
                    .presentation
                    .notice(Notice::MarkerClockFailed(&marker.location)),
            }
        }
        RunOutcome::Completed
    }

    fn run_lanes(&self, request: &RunRequest<'_>, facts: &RunFacts<'_>) -> Vec<LaneReport> {
        let mut reports = Vec::new();
        // Start under the lock, immediately before the first lane. Name order
        // is independent of configuration order, and a failed lane never stops
        // the next one. Each runner receives only the run's remaining budget.
        let run_started = self.clock.start();
        for (name, settings) in request.lanes {
            if request.only.is_some_and(|wanted| wanted != name) {
                continue;
            }
            let budget = lane_budget(settings.deadline, self.clock.elapsed(&run_started));
            if let LaneExecution::Reported(report) =
                self.lanes.execute(name, budget, settings.deadline, facts)
            {
                reports.push(report);
            }
        }
        reports
    }
}

#[cfg(test)]
mod tests;
