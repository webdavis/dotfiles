//! Every expectation was read off the alerter this replaces
//! (`executable_results-alerter.sh`, the complete_records block).

use super::*;

fn split(snapshot: &str) -> (&str, u64) {
    let answer = complete_records(snapshot);
    (answer.text, answer.bytes)
}

#[test]
fn a_snapshot_of_whole_rows_is_taken_entirely() {
    assert_eq!(
        split("{\"a\":1}\n{\"b\":2}\n"),
        ("{\"a\":1}\n{\"b\":2}\n", 16)
    );
}

#[test]
fn a_torn_trailing_line_is_left_for_the_next_run() {
    // osquery writes the row before its newline, so the bytes after the last
    // newline are still being written.
    assert_eq!(split("{\"a\":1}\n{\"b\":"), ("{\"a\":1}\n", 8));
}

#[test]
fn complete_json_without_its_newline_is_still_torn() {
    // THE EXPENSIVE CASE. Treating this as whole would page now and page again
    // once the newline lands, and the second page would cover bytes the first
    // already did.
    assert_eq!(split("{\"a\":1}\n{\"b\":2}"), ("{\"a\":1}\n", 8));
}

#[test]
fn a_snapshot_with_no_newline_at_all_advances_nothing() {
    for snapshot in ["", "{", "{\"a\":1}"] {
        assert_eq!(split(snapshot), ("", 0), "{snapshot:?}");
    }
}

#[test]
fn a_blank_line_is_a_complete_record_as_far_as_the_cursor_is_concerned() {
    // The cursor's job is byte position, not validity. A blank line yields no
    // finding downstream, but retaining it would stall the cursor forever.
    assert_eq!(split("\n"), ("\n", 1));
    assert_eq!(split("{\"a\":1}\n\n"), ("{\"a\":1}\n\n", 9));
}

#[test]
fn the_byte_count_follows_the_file_and_not_the_character_count() {
    // osquery rows carry paths, and a path carries multi-byte characters. A
    // character count would drift from the cursor's byte offset on the first
    // one of them.
    let snapshot = "{\"path\":\"/Users/é/Ünicode/日本\"}\n";
    let (text, bytes) = split(snapshot);
    assert_eq!(text, snapshot);
    assert_eq!(bytes, snapshot.len() as u64);
    assert!(bytes > snapshot.chars().count() as u64);
}

#[test]
fn a_torn_line_carrying_multi_byte_text_still_lands_on_a_character_boundary() {
    // Slicing a Rust string at a byte that is not a boundary panics, so this
    // pins that the split point is always one past a newline.
    let (text, bytes) = split("{\"a\":\"日\"}\n{\"b\":\"本");
    assert_eq!(text, "{\"a\":\"日\"}\n");
    assert_eq!(bytes, text.len() as u64);
}
