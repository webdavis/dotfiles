//! The per-lane staleness bookkeeping one run leaves behind.
//!
//! ITS OWN FILE BECAUSE IT ANSWERS A DIFFERENT QUESTION from the run around
//! it. `Run::execute` orders one run's work; this counts ACROSS runs, and its
//! whole reason to exist is the lane whose every week is silent by design.
//! `uu_domain::next_streak` decides the next count and when it trips; this reads and
//! publishes it, and alerts on a trip.

use uu_domain::{LaneReport, STALE_AFTER_RUNS, next_streak};

use crate::delivery::send_alert;
use crate::ports::{
    AlertOutcome, AlertTarget, Notice, RunDelivery, RunPresentation, RunState, Streak,
};

/// THE STALENESS BOUND: a lane deferring or failing every week is silent by
/// design (a deferral never alerts on its own, and even a failure's alert says
/// nothing about HOW LONG this has been going on), so nothing else says a lane
/// has gone quiet for good. Tracked PER LANE, across runs, independent of
/// whatever else this run's own verdict says.
pub(crate) fn track_staleness(
    state: &impl RunState,
    delivery: &impl RunDelivery,
    presentation: &impl RunPresentation,
    reports: &[LaneReport],
) {
    for report in reports {
        let succeeded = !report.deferred && report.failures == 0;
        let snapshot = state.streak(&report.name);
        // A STREAK THIS RUN COULD NOT TRUST is never read as zero: zero would
        // silently forgive whatever history the file held, which is the
        // opposite of what a mechanism built to notice a lane going quiet
        // may ever do. Treated as one short of the threshold instead, so a
        // non-success run still gets its chance to trip rather than starting
        // a fresh count nobody asked for.
        let previous = match snapshot.value {
            Streak::Absent => 0,
            Streak::Value(value) => value,
            Streak::Unreadable(why) => {
                send_alert(
                    delivery,
                    presentation,
                    AlertTarget::Lane(&report.name),
                    &format!(
                        "this lane's non-success streak at {} could not be trusted ({why}); \
                         treating it as already close to stale rather than silently starting \
                         over",
                        snapshot.location
                    ),
                );
                STALE_AFTER_RUNS - 1
            }
        };
        let (next, tripped) = next_streak(previous, succeeded);
        // AN UNDELIVERED TRIP IS RETRIED, NEVER LOST. This alert fires once
        // per streak, so an engine that was down for the one run that trips
        // would otherwise leave a deferring lane silent for good: a deferral
        // raises nothing else, and the streak only climbs from here. Holding
        // the count one short of the threshold makes the next run trip again.
        // The count is read by nothing but this trip, so a run spent short of
        // its true value costs no reader anything.
        let recorded = if tripped
            && matches!(
                send_alert(
                    delivery,
                    presentation,
                    AlertTarget::Lane(&report.name),
                    &format!(
                        "no successful run in {STALE_AFTER_RUNS} consecutive attempt(s); the last \
                     one {}",
                        if report.deferred {
                            "deferred"
                        } else {
                            "failed"
                        }
                    ),
                ),
                AlertOutcome::Failed(_)
            ) {
            STALE_AFTER_RUNS - 1
        } else {
            next
        };
        // A WRITE FAILURE IS LOUD, never just an eprintln nobody reads from a
        // headless launchd job: this file IS the mechanism, so losing it
        // silently would be exactly the fail-open this whole capability
        // exists to refuse.
        if let Err(failure) = state.write_streak(&report.name, recorded) {
            let why = &failure.cause;
            presentation.notice(Notice::StreakWriteFailed {
                lane: &report.name,
                failure: &failure,
            });
            send_alert(
                delivery,
                presentation,
                AlertTarget::Lane(&report.name),
                &format!(
                    "this lane's non-success streak at {} could not be recorded ({why}); \
                     staleness tracking for it is unreliable until this is fixed",
                    failure.location
                ),
            );
        }
    }
}
