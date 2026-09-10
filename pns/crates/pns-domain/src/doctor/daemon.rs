use super::pairing::PREFIX;

/// The daemon's own line in the doctor's tail run: whether the clock is
/// running, in the five states it can honestly be in.
///
/// FIVE RATHER THAN FOUR, because "off in the config" is two different facts
/// depending on whether a process is still beating, and the operator who just
/// turned the switch off is standing in exactly that state.
///
/// IT NEVER MOVES THE EXIT CODE, in any state, including the dead one. The
/// doctor's code is what an operator's automation reads as "notifications are
/// broken", and a daemon that is down costs ambient features rather than a
/// card. Reporting it as a broken notifier would be the fail-open sin's
/// mirror: a true alarm about the wrong thing, in a place that already means
/// something else. That is why this returns a String and is never an input to
/// `exit_code`.
///
/// IT COUNTS JOBS AND NEVER NAMES THEM, following the missed journal's
/// structural privacy rule: the count answers "is anything scheduled" and the
/// contents are a reader nobody asked for.
pub fn daemon_line(
    enabled: bool,
    beat: Option<crate::jobs::Heartbeat>,
    now: Option<u64>,
    jobs: usize,
) -> String {
    // AN AGE THAT CANNOT BE TAKEN IS NOT A FRESH BEAT. No clock, and a beat
    // stamped after now, both leave nothing to compare, and vouching for a
    // daemon on the strength of a timestamp nothing could grade is the
    // identity-is-not-presence mistake with a file standing in for the process.
    let age = beat.and_then(|beat| now.and_then(|now| now.checked_sub(beat.at)));
    let beating = age.is_some_and(|age| age <= crate::jobs::HEARTBEAT_STALE_SECS);
    if !enabled {
        // THE CONFIG IS NOT THE PROCESS. Nothing bounces the launchd job when
        // the config changes, so a daemon started while the switch was on keeps
        // running after it is turned off, and it keeps firing jobs. Reporting
        // it as simply off would be this line saying the opposite of the truth
        // in the one state an operator turned the switch to reach.
        return match beat {
            Some(beat) if beating => format!(
                "{PREFIX}the daemon is off in the config, but pid {} is still beating; \
                 bootout (or wait) to stop it",
                beat.pid
            ),
            _ => format!("{PREFIX}the daemon is off in the config"),
        };
    }
    let Some(beat) = beat else {
        return format!("{PREFIX}the daemon is enabled and has not run yet");
    };
    match age {
        Some(age) if age <= crate::jobs::HEARTBEAT_STALE_SECS => format!(
            "{PREFIX}the daemon is running, pid {}, {jobs} job{} scheduled",
            beat.pid,
            if jobs == 1 { "" } else { "s" }
        ),
        Some(age) => format!(
            "{PREFIX}the daemon is enabled, its last beat was {}, so it is not running",
            super::ago(age)
        ),
        None => format!(
            "{PREFIX}the daemon is enabled, its last beat was an unknown time ago, \
             so it is not running"
        ),
    }
}

/// The nag's own line in the doctor's tail run: what the schedule is, said in
/// the unit the card will say it in.
///
/// IT REPORTS THE CONFIG ONLY AND DOES NOT GRADE THE DAEMON. A nag with a dead
/// daemon never fires, which is a true and important thing to say, but
/// `daemon_line` one row above already says the daemon is not running, from the
/// heartbeat, and two lines deriving one fact is how they drift apart. THE
/// PLACEMENT IS THE WHOLE MITIGATION: the two read as one paragraph.
///
/// IT DOES NOT MOVE THE EXIT CODE, for `focus_line`'s reason: a nag being off
/// is not a fault, and the doctor's exit code is what an operator's automation
/// reads as "notifications are broken".
///
/// TWO STATES AND NOT THREE. No table and `after_secs = 0` are the SAME
/// statement in this config, so telling them apart here would be the doctor
/// inventing a distinction the parser does not carry.
pub fn nag_line(after_secs: u64) -> String {
    match after_secs {
        0 => format!("{PREFIX}the nag is off (no `[nag] after_secs`)"),
        seconds => format!(
            "{PREFIX}an unanswered approval is carded again after {}",
            crate::nag::waited(seconds)
        ),
    }
}

#[cfg(test)]
mod nag_tests {
    use super::nag_line;

    #[test]
    fn the_nag_line_names_the_schedule_or_says_the_feature_is_off() {
        assert_eq!(
            nag_line(0),
            "pns doctor: the nag is off (no `[nag] after_secs`)"
        );
        assert_eq!(
            nag_line(300),
            "pns doctor: an unanswered approval is carded again after 5m"
        );
        // THE SAME UNIT THE CARD USES, so "carded again after 30s" and "still
        // waiting 30s" are one operator reading one number twice.
        assert_eq!(
            nag_line(30),
            "pns doctor: an unanswered approval is carded again after 30s"
        );
    }
}

#[cfg(test)]
mod daemon_tests {
    use super::daemon_line;
    use crate::jobs::{HEARTBEAT_STALE_SECS, Heartbeat, parse_heartbeat, render_heartbeat};

    const NOW: u64 = 1_700_000_000;

    /// Four of the five states, each with its own sentence. The fifth, a switch
    /// turned off while the process is still beating, is next door, because it
    /// is the one an operator reaches by ACTING rather than by waiting.
    ///
    /// NONE OF THEM MOVES THE EXIT CODE, which is asserted where the exit code
    /// is decided: `exit_code` takes outcomes and a pairing report and this
    /// line is neither, so a dead daemon structurally cannot read as a broken
    /// notifier. The binary suite pins the same thing end to end.
    #[test]
    fn the_daemons_doctor_line_tells_the_truth_in_four_states() {
        assert_eq!(
            daemon_line(false, None, Some(NOW), 0),
            "pns doctor: the daemon is off in the config"
        );
        assert_eq!(
            daemon_line(true, None, Some(NOW), 0),
            "pns doctor: the daemon is enabled and has not run yet"
        );
        let stale = Heartbeat {
            pid: 4321,
            at: NOW - HEARTBEAT_STALE_SECS - 1,
        };
        assert_eq!(
            daemon_line(true, Some(stale), Some(NOW), 2),
            format!(
                "pns doctor: the daemon is enabled, its last beat was {}, \
                 so it is not running",
                crate::doctor::ago(HEARTBEAT_STALE_SECS + 1)
            )
        );
        let fresh = Heartbeat {
            pid: 4321,
            at: NOW - HEARTBEAT_STALE_SECS,
        };
        assert_eq!(
            daemon_line(true, Some(fresh), Some(NOW), 2),
            "pns doctor: the daemon is running, pid 4321, 2 jobs scheduled"
        );
        assert_eq!(
            daemon_line(true, Some(fresh), Some(NOW), 1),
            "pns doctor: the daemon is running, pid 4321, 1 job scheduled"
        );
    }

    /// S206: A TICK LONGER THAN THE BOUND READS A HEALTHY DAEMON AS DEAD.
    ///
    /// `HEARTBEAT_STALE_SECS` is ten of the DEFAULT tick and is fixed at
    /// compile time, while `PNS_DAEMON_TICK_MS` is read at run time and admits
    /// far more. On any tick longer than the bound the previous beat is
    /// ALREADY STALE when the next one is written, so a daemon that is beating
    /// exactly as configured grades as not running, on every tick, forever.
    ///
    /// DRIVEN THROUGH `daemon_line` AND NOTHING ELSE. An earlier version of
    /// this test compared two numbers against a copy of the rule, which
    /// established the constant's value and nothing about the grader: widening
    /// the classifier left it green.
    #[test]
    fn a_tick_slower_than_the_bound_grades_a_beating_daemon_as_not_running() {
        // One beat per thirty seconds, which is a daemon told to look every
        // thirty seconds and doing so.
        let tick_secs = 30;
        let beating_as_told = Heartbeat {
            pid: 4321,
            at: NOW - tick_secs,
        };
        assert_eq!(
            daemon_line(true, Some(beating_as_told), Some(NOW), 0),
            format!(
                "pns doctor: the daemon is enabled, its last beat was {}, \
                 so it is not running",
                crate::doctor::ago(tick_secs)
            ),
            "a {tick_secs}s tick outruns the {HEARTBEAT_STALE_SECS}s bound"
        );
        // AND THE SAME DAEMON ON THE DEFAULT TICK GRADES AS RUNNING, which is
        // what makes the line above a statement about the bound rather than
        // about this fixture.
        let beating_at_the_default = Heartbeat {
            pid: 4321,
            at: NOW - 1,
        };
        assert_eq!(
            daemon_line(true, Some(beating_at_the_default), Some(NOW), 0),
            "pns doctor: the daemon is running, pid 4321, 0 jobs scheduled"
        );
    }

    /// OFF IN THE CONFIG IS NOT THE SAME FACT AS STOPPED.
    ///
    /// Nothing bounces the launchd job when the config changes, so a daemon
    /// started while the switch was on keeps running and keeps firing after it
    /// is turned off. The operator who just flipped it is standing in exactly
    /// that state, and a line that said only "off in the config" would be
    /// telling them the opposite of the truth at the one moment they looked.
    #[test]
    fn a_daemon_switched_off_but_still_beating_is_reported_as_still_beating() {
        let beating = Heartbeat { pid: 991, at: NOW };
        assert_eq!(
            daemon_line(false, Some(beating), Some(NOW), 3),
            "pns doctor: the daemon is off in the config, but pid 991 is still beating; \
             bootout (or wait) to stop it"
        );
        // A BEAT TOO OLD TO VOUCH FOR IS NOT A RUNNING PROCESS, so the plain
        // sentence is what an operator who stopped it days ago still reads.
        let stale = Heartbeat {
            pid: 991,
            at: NOW - HEARTBEAT_STALE_SECS - 1,
        };
        assert_eq!(
            daemon_line(false, Some(stale), Some(NOW), 0),
            "pns doctor: the daemon is off in the config"
        );
    }

    /// A beat this machine cannot grade is NOT RUNNING rather than running.
    ///
    /// FAIL TOWARDS THE HONEST REPORT: no clock, or a beat stamped in the
    /// future, both mean the age is not a number, and claiming a daemon is
    /// alive on the strength of a timestamp nothing could compare is the
    /// identity-is-not-presence mistake with a file standing in for the pid.
    #[test]
    fn a_heartbeat_whose_age_cannot_be_taken_reads_as_not_running() {
        let beat = Heartbeat { pid: 7, at: NOW };
        for (case, now) in [
            ("no clock", None),
            ("a beat from the future", Some(NOW - 5)),
        ] {
            let line = daemon_line(true, Some(beat), now, 0);
            assert!(
                line.contains("not running") && line.contains("an unknown time"),
                "{case}: {line}"
            );
        }
    }

    /// The heartbeat file's own round trip, since the doctor's whole reading
    /// arrives through it.
    #[test]
    fn a_heartbeat_round_trips_and_anything_else_is_no_heartbeat_at_all() {
        let beat = Heartbeat { pid: 4321, at: NOW };
        assert_eq!(parse_heartbeat(&render_heartbeat(&beat)), Some(beat));
        for not_a_beat in ["", "4321", "4321 soon", "nobody 1700000000", "0 1700000000"] {
            assert_eq!(parse_heartbeat(not_a_beat), None, "case: {not_a_beat}");
        }
    }
}
