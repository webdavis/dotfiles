//! `uu run [<lane>]`: every enabled lane, or just one.
//!
//! THE ORDER IS THE DESIGN. The lock comes first, then the pruning under it,
//! then the header is captured BEFORE the lanes run and before the marker is
//! rewritten (a gap sampled at delivery reports zero on every run), then the
//! lanes, then the alerts, then the staleness bookkeeping, then the record,
//! and only a wholly clean run moves the marker.

use std::time::Instant;

use pns::channels::hermes::UreqSignedPost;
use unattended_upgrades::alert::alert_summary;
use unattended_upgrades::config::{Config, config_path};
use unattended_upgrades::deadline::lane_budget;
use unattended_upgrades::lanes::{LaneReport, run_lane};
use unattended_upgrades::record::{RunFacts, gap_line, record_body, record_detail, record_state};
use unattended_upgrades::staleness::{STALE_AFTER_RUNS, next_streak};

use crate::delivery::{PnsAlerter, deliver_record, send_alert};
use crate::runner::SystemRunner;
use crate::state::streak::{self, Streak};
use crate::state::{lock, marker};
use crate::system::{home, host, iso, now_epoch};

pub fn run_mode(only: Option<&str>) -> i32 {
    let Some(home) = home() else {
        return super::no_home();
    };

    let path = config_path(&home);
    let config = match super::loaded(&path) {
        Ok(Some(config)) => config,
        Ok(None) => {
            // A bare run on a configless machine is clean by design. A lane
            // asked for BY NAME is a request, and one no file declares did
            // not run, so it is refused the way an undeclared name is below.
            if let Some(lane) = only {
                eprintln!(
                    "uu: no config at {}, so no lane `{lane}` is declared",
                    path.display()
                );
                return 1;
            }
            println!(
                "uu: no config at {}; nothing is enabled and nothing was updated",
                path.display()
            );
            return 0;
        }
        Err(code) => return code,
    };

    let _lock = match lock::acquire(&home) {
        Ok(lock) => lock,
        Err(lock::LockFailure::Contended(why)) => {
            eprintln!("uu: {why}; not running, to avoid racing the run that already holds it");
            return 1;
        }
        Err(lock::LockFailure::Unavailable(why)) => {
            eprintln!("uu: {why}; not running");
            return 1;
        }
    };

    // A LANE NO LONGER DECLARED IS PRUNED HERE, under the same lock: its
    // directory (and whatever streak it held) would otherwise leak forever,
    // and a NEW lane that reuses the old name would inherit that streak and
    // could alert on its very first miss.
    streak::prune_removed_lanes(&home, &config);

    // A clock that cannot be read renders as the epoch itself, which is a date
    // no reader mistakes for a real one.
    let started = now_epoch().unwrap_or(0);
    let started_iso = iso(started);
    let marker_path = marker::path(&home);
    let marker = marker::read(&marker_path);
    let gap = gap_line(&marker, &marker_path.display().to_string(), started);
    let host_name = host();
    let facts = RunFacts {
        host: &host_name,
        started_epoch: started,
        started_iso: &started_iso,
        marker: &marker,
    };

    let reports = run_lanes(&config, &facts, only);
    if let Some(lane) = only
        && reports.is_empty()
    {
        eprintln!(
            "uu: lane `{lane}` has no `[lanes.{lane}]` block in {}",
            path.display()
        );
        return 1;
    }

    let failures: usize = reports.iter().map(|report| report.failures).sum();
    let deferred: usize = reports.iter().filter(|report| report.deferred).count();
    let detail = record_detail(&host_name, &started_iso, &gap, &reports);
    print!("{detail}");

    let engine = config.alerts.as_ref().map(|alerts| alerts.binary.clone());
    for report in reports.iter().filter(|report| report.failures > 0) {
        send_alert(
            &PnsAlerter,
            engine.as_deref(),
            &report.name,
            &alert_summary(report),
        );
    }

    track_staleness(&home, &reports, engine.as_deref());

    // A RECORD THE GATEWAY NEVER RECEIVED IS A FAILED RUN, even when every
    // lane passed. The entry is the whole point of the week's work, and the
    // marker is what the NEXT entry measures its gap from: stamping a success
    // nothing recorded would have that entry claim a gap from a week no one
    // can read. With no `[records]` block nothing was owed, so nothing is
    // lost.
    let record_lost = match config.records.as_ref() {
        Some(records) => !deliver_record(
            &UreqSignedPost,
            &PnsAlerter,
            records,
            records_body(failures, deferred, &detail),
            engine.as_deref(),
        ),
        None => {
            println!("uu: no [records] block; this run was logged here and nowhere else");
            false
        }
    };

    // THE MARKER MOVES ONLY ON A CLEAN RUN, so the next entry's gap measures
    // the last time everything actually worked rather than the last time uu
    // woke up.
    //
    // A DEFERRAL IS NOT A CLEAN RUN EITHER. A deferred lane did no work, so it
    // must not count as the run that advances the marker: the marker means
    // the last time everything actually ran and succeeded, and letting a
    // deferral through would have a lane that never runs read as healthy
    // forever, which is exactly the failure mode this verdict exists to make
    // visible instead.
    //
    // AND IT STAMPS THE MOMENT THE RUN FINISHED, read here rather than reused
    // from the header. Lanes have no upper bound, so the header's instant can
    // be an hour old by now, and every following gap would carry that hour on
    // top of its own. A clock that will not answer at this instant leaves the
    // marker alone: an unmoved marker overstates the next gap, while a
    // guessed timestamp understates it silently.
    if failures == 0 && deferred == 0 && !record_lost {
        match now_epoch() {
            Some(finished) => marker::write(&marker_path, finished),
            None => eprintln!(
                "uu: this clock could not be read at the end of the run, so the successful-run \
                 timestamp at {} was left as it was; the next entry measures its gap from the \
                 run before this one",
                marker_path.display()
            ),
        }
    }
    0
}

/// Every declared lane, or the one asked for by name.
///
/// IN NAME ORDER, never the file's: `lanes` is a `BTreeMap`, and a run whose
/// sequence changes when a block moves is a run nobody can reason about.
///
/// CONTINUE ON FAILURE: nothing here inspects a report before moving to the
/// next lane.
fn run_lanes(config: &Config, facts: &RunFacts, only: Option<&str>) -> Vec<LaneReport> {
    let mut reports = Vec::new();
    // THE RUN'S OWN CLOCK, started under the lock this loop holds. Lanes run in
    // sequence and nothing caps how many a config declares, so each lane's
    // budget is capped by what is left of the run's (`lane_budget`).
    let run_started = Instant::now();
    for (name, lane) in &config.lanes {
        if only.is_some_and(|wanted| wanted != name) {
            continue;
        }
        // ONE RUNNER PER LANE, holding that lane's own budget: it is the whole
        // lane's, and its clock starts here.
        let runner = SystemRunner::for_lane(
            name,
            lane_budget(lane.deadline, run_started.elapsed()),
            lane.deadline,
        );
        if let Some(report) = run_lane(name, config, facts, &runner) {
            reports.push(report);
        }
    }
    reports
}

/// THE STALENESS BOUND: a lane deferring or failing every week is silent by
/// design (a deferral never alerts on its own, and even a failure's alert says
/// nothing about HOW LONG this has been going on), so nothing else says a lane
/// has gone quiet for good. Tracked PER LANE, across runs, independent of
/// whatever else this run's own verdict says.
fn track_staleness(home: &str, reports: &[LaneReport], engine: Option<&str>) {
    for report in reports {
        let succeeded = !report.deferred && report.failures == 0;
        let path = streak::path(home, &report.name);
        // A STREAK THIS RUN COULD NOT TRUST is never read as zero: zero would
        // silently forgive whatever history the file held, which is the
        // opposite of what a mechanism built to notice a lane going quiet
        // may ever do. Treated as one short of the threshold instead, so a
        // non-success run still gets its chance to trip rather than starting
        // a fresh count nobody asked for.
        let previous = match streak::read(&path) {
            Streak::Absent => 0,
            Streak::Value(value) => value,
            Streak::Unreadable(why) => {
                send_alert(
                    &PnsAlerter,
                    engine,
                    &report.name,
                    &format!(
                        "this lane's non-success streak at {} could not be trusted ({why}); \
                         treating it as already close to stale rather than silently starting \
                         over",
                        path.display()
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
            && !send_alert(
                &PnsAlerter,
                engine,
                &report.name,
                &format!(
                    "no successful run in {STALE_AFTER_RUNS} consecutive attempt(s); the last \
                     one {}",
                    if report.deferred {
                        "deferred"
                    } else {
                        "failed"
                    }
                ),
            ) {
            STALE_AFTER_RUNS - 1
        } else {
            next
        };
        // A WRITE FAILURE IS LOUD, never just an eprintln nobody reads from a
        // headless launchd job: this file IS the mechanism, so losing it
        // silently would be exactly the fail-open this whole capability
        // exists to refuse.
        if let Err(why) = streak::write(&path, recorded) {
            eprintln!(
                "uu: could not record lane `{}`'s non-success streak at {}: {why}",
                report.name,
                path.display()
            );
            send_alert(
                &PnsAlerter,
                engine,
                &report.name,
                &format!(
                    "this lane's non-success streak at {} could not be recorded ({why}); \
                     staleness tracking for it is unreliable until this is fixed",
                    path.display()
                ),
            );
        }
    }
}

fn records_body(failures: usize, deferred: usize, detail: &str) -> String {
    record_body(record_state(failures, deferred), &host(), detail)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_deferred_only_run_posts_a_body_stated_deferred_not_completed() {
        // record_state itself is pinned directly in record.rs; this instead
        // guards the CALL SITE here in `records_body`, where a mutant
        // passing `record_state(failures, 0)` would post every deferred-only
        // run as "completed" while leaving every `record_state` unit test
        // green.
        let body = records_body(0, 1, "detail");
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["state"], "deferred");
    }

    #[test]
    fn a_mixed_run_posts_a_body_stated_failed_not_deferred() {
        let body = records_body(1, 1, "detail");
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["state"], "failed");
    }
}
