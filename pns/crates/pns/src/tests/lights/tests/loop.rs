//! The lamps, pinned: loop.

use super::fixtures::*;

// --- the loop lamp ------------------------------------------------------

const THRESHOLD: u64 = 360;
const LEASE_TIMEOUT: u64 = 3_900;

/// One reading, with everything not under test set to nothing happening.
fn running<'reading>(
    streak: Option<&'reading Streak>,
    agents_working: bool,
    leases: &'reading [u64],
) -> Loop<'reading> {
    Loop {
        streak,
        agents_working,
        shell_since: None,
        leases,
        now: NOW,
        threshold_secs: THRESHOLD,
        lease_timeout_secs: LEASE_TIMEOUT,
    }
}

#[test]
fn a_shell_command_is_measured_from_its_own_start_and_not_from_an_agents_streak() {
    // TWO SOURCES, TWO CLOCKS, and one shared streak could not serve both.
    // The shell publishes the second its command STARTED, which is an exact
    // start nothing has to infer; an agent gives a status word and nothing
    // else, so its run is timed from the first tick that read it working.
    //
    // POOLED, THEY BORROWED EACH OTHER'S TIME IN BOTH DIRECTIONS. The
    // streak outlives the work by the grace that covers an agent's turn
    // gap, so a fresh five-second command starting inside that grace
    // inherited the streak and armed the lamp at once; and a build that had
    // already been running for ten minutes when the streak was empty was
    // clocked from now and had to wait out the whole threshold again.
    let stale = Streak {
        since: NOW - 5_000,
        last_seen: NOW - 60,
    };
    assert!(
        !loop_running(&Loop {
            shell_since: Some(NOW - 5),
            ..running(Some(&stale), false, &[])
        }),
        "a five-second command cannot inherit an agent's finished run"
    );
    assert!(
        loop_running(&Loop {
            shell_since: Some(NOW - THRESHOLD),
            ..running(None, false, &[])
        }),
        "and a build already past the threshold arms from its OWN start, \
         with no streak behind it and nothing to wait out again"
    );
    assert!(
        !loop_running(&Loop {
            shell_since: Some(NOW - THRESHOLD + 1),
            ..running(None, false, &[])
        }),
        "one second under it is not a loop yet: the same closed edge"
    );
    // AND THE AGENT'S OWN RUN IS NOT DESTROYED BY A FRESH COMMAND, which is
    // the mirror of the first case and the reason this is two readings
    // rather than one taken over the earlier of them.
    let long = streak_from(THRESHOLD);
    assert!(
        loop_running(&Loop {
            shell_since: Some(NOW),
            ..running(Some(&long), true, &[])
        }),
        "an agent ten minutes in keeps its lamp when somebody runs `ls`"
    );
    // A CLOCK BEHIND THE COMMAND HAS NO ELAPSED TIME IN IT.
    assert!(
        !loop_running(&Loop {
            shell_since: Some(NOW + 500),
            ..running(None, false, &[])
        }),
        "a now before the command started has no elapsed time in it"
    );
}

#[test]
fn work_past_the_threshold_arms_the_loop_lamp_and_both_edges_are_closed() {
    let under = streak_from(THRESHOLD - 1);
    let at = streak_from(THRESHOLD);
    assert!(
        !loop_running(&running(Some(&under), true, &[])),
        "one second under the threshold is not a loop yet"
    );
    assert!(
        loop_running(&running(Some(&at), true, &[])),
        "exactly at it, it arms"
    );
    assert!(
        !loop_running(&running(None, true, &[])),
        "work with no streak behind it has no duration to measure"
    );
    // BOTH HALVES, which is the condition as written: something is working
    // AND the run is old enough. The streak deliberately OUTLIVES the work
    // by the grace that covers the gap between a loop's turns, so a reading
    // of the streak alone keeps claiming work in progress for minutes after
    // everything went idle.
    assert!(
        !loop_running(&running(Some(&at), false, &[])),
        "a streak still inside its grace is not work that is still running"
    );
    // A CLOCK BEHIND THE STREAK IS NOT A LONG RUN.
    let future = Streak {
        since: NOW + 500,
        last_seen: NOW + 500,
    };
    assert!(
        !loop_running(&running(Some(&future), true, &[])),
        "a now before the streak began has no elapsed time in it"
    );
}

#[test]
fn a_live_lease_arms_the_loop_lamp_with_nothing_working_and_an_expired_one_does_not() {
    let idle = streak_from(0);
    assert!(
        loop_running(&running(None, false, &[NOW - LEASE_TIMEOUT])),
        "exactly at the timeout is still live: both edges closed"
    );
    assert!(
        !loop_running(&running(None, false, &[NOW - LEASE_TIMEOUT - 1])),
        "one second past it, an abandoned lease can no longer hold the lamp"
    );
    assert!(
        loop_running(&running(Some(&idle), false, &[NOW - 5_000, NOW])),
        "one live lease among expired ones is enough, and it needs no work behind it"
    );
    assert!(
        !loop_running(&running(None, false, &[])),
        "and no lease at all with nothing working is a dark lamp"
    );
}

#[test]
fn a_pane_id_that_cannot_be_a_filename_names_no_lease_at_all() {
    let state = std::path::Path::new("/state");
    assert_eq!(
        lease_marker(state, "wW:p21"),
        Some(state.join("lights-loop").join("wW:p21")),
        "herdr's own id names a file inside the lease directory, colon and all"
    );
    for refused in ["..", "../etc/passwd", "a/b", "", "a b"] {
        assert_eq!(
            lease_marker(state, refused),
            None,
            "{refused:?} must name no lease"
        );
    }
}
