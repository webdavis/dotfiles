use super::*;

const CURSOR: &str = "Thu, 25 Oct 2026 15:16:27 GMT";

fn state() -> PollState {
    PollState {
        last_modified: CURSOR.to_string(),
        interval_secs: 60,
        seen: vec![
            Seen {
                identity: "webdavis/dotfiles|workflow_run|4471".to_string(),
                first_seen: 1_700_000_000,
            },
            Seen {
                identity: "webdavis/dotfiles|review_request|689".to_string(),
                first_seen: 1_700_000_060,
            },
        ],
    }
}

#[test]
fn what_the_writer_renders_is_what_the_reader_parses() {
    // THE ROUND TRIP IS THE PROPERTY, over the empty state, the cursor's
    // spaces and commas, and a full seen-set.
    for poll in [PollState::default(), state()] {
        assert_eq!(
            parse_poll_state(&render_poll_state(&poll)),
            poll,
            "{poll:?}"
        );
    }
}

#[test]
fn a_trailing_newline_is_no_extra_identity() {
    // `publish_state_line` appends one, so the reader sees it on every real
    // file: an empty line read as an identity would be an entry nothing ever
    // matches, forever.
    let text = format!("{}\n", render_poll_state(&state()));
    assert_eq!(parse_poll_state(&text), state());
}

#[test]
fn a_file_that_is_not_this_format_is_no_state_rather_than_a_partial_one() {
    // THE MUTANT THIS PINS: a half-read state, which is a cursor without the
    // interval it came with, or a seen-set with no cursor.
    for text in ["", "nonsense", "not-a-count Thu, 25 Oct 2026 15:16:27 GMT"] {
        assert_eq!(
            parse_poll_state(text),
            PollState::default(),
            "case {text:?}"
        );
    }
}

#[test]
fn a_malformed_identity_line_is_dropped_and_the_others_survive() {
    let text = format!("60 {CURSOR}\nnot-a-count id-a\n1700000000\n1700000001 id-b\n");
    let parsed = parse_poll_state(&text);
    assert_eq!(parsed.interval_secs, 60);
    assert_eq!(parsed.last_modified, CURSOR);
    assert_eq!(parsed.seen.len(), 1);
    assert_eq!(parsed.seen[0].identity, "id-b");
}

#[test]
fn a_cursor_with_no_date_is_still_the_interval_it_states() {
    // The first answer after a fresh start has an interval and no cursor yet.
    let parsed = parse_poll_state("60 \n");
    assert_eq!(parsed.interval_secs, 60);
    assert_eq!(parsed.last_modified, "");
}

#[test]
fn a_field_that_would_forge_a_line_is_never_published() {
    // The cursor and the identities are REMOTE TEXT: a newline in either
    // would change what the next read believes, so neither reaches the file.
    let hostile = PollState {
        last_modified: "Thu, 25 Oct\n1700000000 forged".to_string(),
        interval_secs: 60,
        seen: vec![
            Seen {
                identity: "a\nb".to_string(),
                first_seen: 1,
            },
            Seen {
                identity: String::new(),
                first_seen: 2,
            },
            Seen {
                identity: "kept".to_string(),
                first_seen: 3,
            },
        ],
    };
    let rendered = render_poll_state(&hostile);
    assert_eq!(rendered, "60 \n3 kept");
    let parsed = parse_poll_state(&rendered);
    assert_eq!(parsed.last_modified, "");
    assert_eq!(parsed.seen.len(), 1);
    assert_eq!(parsed.seen[0].identity, "kept");
}

#[test]
fn a_published_state_reads_back_out_of_the_directory() {
    let home = crate::state_fixtures::scratch("github-poll-publish");
    write_poll_state(&home, &state()).expect("it publishes");
    assert_eq!(read_poll_state(&home), state());
}

#[test]
fn a_directory_with_no_state_file_reads_the_empty_state() {
    // THE MUTANT THIS PINS: a read that failed being reported as anything but
    // the empty state, which would stop the first poll on every fresh machine.
    assert_eq!(
        read_poll_state(&crate::state_fixtures::scratch("github-poll-absent")),
        PollState::default()
    );
}

#[test]
fn a_state_file_larger_than_this_reads_is_the_empty_state_rather_than_read() {
    let home = crate::state_fixtures::scratch("github-poll-oversized");
    std::fs::write(
        home.join(GITHUB_POLL_STATE),
        "x".repeat(GITHUB_POLL_READ_MAX as usize + 1),
    )
    .expect("the oversized file is written");
    assert_eq!(read_poll_state(&home), PollState::default());
}
