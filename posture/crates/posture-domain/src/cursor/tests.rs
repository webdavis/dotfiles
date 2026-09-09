//! Every expectation was read off the alerter this replaces
//! (`executable_results-alerter.sh`, the cursor block), not invented.

use super::*;

fn stored(inode: u64, offset: u64) -> Option<StoredCursor> {
    Some(StoredCursor { inode, offset })
}

fn live(inode: u64, size: u64) -> LiveLog {
    LiveLog { inode, size }
}

#[test]
fn a_log_that_has_grown_is_read_from_where_the_cursor_stopped() {
    assert_eq!(
        advance(stored(7, 100), live(7, 250)),
        Advance::Read {
            from: 100,
            reset: false
        }
    );
}

#[test]
fn a_log_that_has_not_grown_is_not_read_and_the_cursor_is_not_rewritten() {
    // A tick that rewrote an unchanged cursor would turn every quiet minute
    // into a write, and a crash mid-write would lose a correct position.
    assert_eq!(advance(stored(7, 250), live(7, 250)), Advance::Nothing);
}

#[test]
fn a_rotated_log_is_read_from_the_top_rather_than_the_old_offset() {
    // The recorded offset points into a file that is gone, so reading from it
    // would skip real rows.
    assert_eq!(
        advance(stored(7, 900), live(8, 250)),
        Advance::Read {
            from: 0,
            reset: false
        }
    );
}

#[test]
fn a_truncated_log_is_read_from_the_top_rather_than_past_its_end() {
    assert_eq!(
        advance(stored(7, 900), live(7, 250)),
        Advance::Read {
            from: 0,
            reset: false
        }
    );
}

#[test]
fn a_rotation_that_lands_on_the_same_size_as_the_old_offset_is_still_read() {
    // The rotation rule runs BEFORE the nothing-to-do check, so a new file
    // whose size happens to equal the old offset is not mistaken for a log
    // that has not grown.
    assert_eq!(
        advance(stored(7, 250), live(8, 250)),
        Advance::Read {
            from: 0,
            reset: false
        }
    );
}

#[test]
fn a_lost_cursor_replays_the_whole_log_and_says_so() {
    // DELETING THE CURSOR MUST NOT SUPPRESS A BATCH. Seeking quietly to the end
    // would make one `rm` a way to hide a queued finding.
    assert_eq!(
        advance(None, live(7, 250)),
        Advance::Read {
            from: 0,
            reset: true
        }
    );
}

#[test]
fn a_lost_cursor_on_an_empty_log_is_quiet_rather_than_alarming() {
    // The nothing-to-do check comes LAST, so a reset with nothing to replay
    // raises no alarm. The alerter this replaces exits before its own warning
    // for the same reason.
    assert_eq!(advance(None, live(7, 0)), Advance::Nothing);
}

#[test]
fn a_cursor_is_two_numbers_and_survives_a_missing_trailing_newline() {
    // CAPTURE THEN VALIDATE: a file without its newline still yields both
    // fields, and treating that as a failure would skip a whole batch.
    assert_eq!(parse("7 250"), stored(7, 250));
    assert_eq!(parse("7 250\n"), stored(7, 250));
    assert_eq!(parse("  7   250  \n"), stored(7, 250));
}

#[test]
fn anything_that_is_not_two_numbers_is_not_a_cursor() {
    for text in [
        "",
        "\n",
        "7",
        "7 ",
        "seven 250",
        "7 two-fifty",
        "-1 250",
        "7 250 extra",
    ] {
        assert_eq!(parse(text), None, "{text:?}");
    }
}

#[test]
fn a_cursor_survives_the_round_trip_it_is_written_for() {
    let original = StoredCursor {
        inode: 12_345,
        offset: 67_890,
    };
    assert_eq!(parse(&render(original)), Some(original));
}
