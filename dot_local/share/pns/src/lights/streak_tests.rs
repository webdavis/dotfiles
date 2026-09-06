//! The lamps, pinned: streak.

use super::fixtures::*;

#[test]
fn every_workspaces_agent_status_is_read_and_a_missing_one_is_not_working() {
    assert_eq!(
        workspace_agent_statuses(HERDR_WORKSPACES),
        vec![WORKING, "idle", "unknown"],
        "herdr's real answer, in its own order"
    );
    assert_eq!(
        workspace_agent_statuses(NO_STATUS_FIELD),
        vec![String::new()],
        "a workspace with no agent_status is a workspace this will not call working"
    );
    assert!(
        workspace_agent_statuses("not json").is_empty(),
        "an unreadable answer names no working workspace"
    );
}

#[test]
fn one_working_workspace_is_enough_and_none_of_them_working_is_not() {
    let statuses =
        |words: &[&str]| -> Vec<String> { words.iter().map(|word| word.to_string()).collect() };
    assert!(
        any_working(&statuses(&["idle", WORKING, "unknown"]), None),
        "the operator's rule, applied literally: breathing if AT LEAST ONE thing is working"
    );
    assert!(
        !any_working(&statuses(&["idle", "unknown", "blocked"]), None),
        "blocked is the operator's turn, not a loop running, so nothing here is working"
    );
    assert!(
        !any_working(&[], None),
        "no workspace at all is nothing working"
    );
    assert!(
        any_working(&statuses(&["idle"]), Some(1_000)),
        "and a plain long shell command is a working loop with no workspace behind it"
    );
}

#[test]
fn the_streak_starts_survives_a_gap_between_turns_and_clears_behind_the_grace() {
    const GRACE: u64 = 120;
    let held = Streak {
        since: 1_000,
        last_seen: 1_050,
    };
    assert_eq!(
        next_streak(None, true, 1_000, GRACE),
        Some(Streak {
            since: 1_000,
            last_seen: 1_000
        }),
        "working with no streak starts one at now"
    );
    assert_eq!(
        next_streak(Some(held.clone()), true, 1_200, GRACE),
        Some(Streak {
            since: 1_000,
            last_seen: 1_200
        }),
        "working with a streak keeps its START and only moves what it last saw"
    );
    // THE CASE THAT MATTERS. The seconds between a loop's turns read as
    // not-working, and a streak that reset there could never reach a
    // threshold measured in minutes.
    assert_eq!(
        next_streak(Some(held.clone()), false, 1_050 + GRACE, GRACE),
        Some(held.clone()),
        "not working INSIDE the grace leaves the streak exactly as it was"
    );
    assert_eq!(
        next_streak(Some(held.clone()), false, 1_050 + GRACE + 1, GRACE),
        None,
        "and one second past the grace clears it"
    );
    assert_eq!(
        next_streak(None, false, 1_000, GRACE),
        None,
        "nothing working and no streak stays nothing"
    );
}

#[test]
fn a_streak_survives_as_one_line_and_anything_else_is_no_streak() {
    let held = Streak {
        since: 1_000,
        last_seen: 1_200,
    };
    assert_eq!(render_streak(&held), "1000 1200");
    assert_eq!(parse_streak("1000 1200"), Some(held));
    // REFUSED, NEVER GUESSED AT, in `parse_heartbeat`'s style: a file some
    // other hand rewrote is not a streak, and reading half of one as zero
    // would report a loop as having worked since 1970.
    for garbled in [
        "",
        "1000",
        "1000 1200 1400",
        "x 1200",
        "1000 x",
        " 1000 1200",
    ] {
        assert_eq!(parse_streak(garbled), None, "{garbled:?} is not a streak");
    }
}
