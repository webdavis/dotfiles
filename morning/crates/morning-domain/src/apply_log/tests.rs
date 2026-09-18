use super::*;

/// A transcript this repository's own apply wrote, trimmed to its shape.
const TRANSCRIPT: &str = "chezmoi apply\n\
started:    2026-09-17T10:17:26Z\n\
repo:       /Users/stephen/workspaces/dotfiles\n\
git HEAD:   8f108c1c\n\
\n\
apply output:\n\
pns engine\n\
installed /Users/stephen/.cargo/bin/pns\n\
\n\
finished:   2026-09-17T10:18:49Z\n\
exit_code:  0\n\
result:     OK\n";

#[test]
fn reads_the_result_and_the_finish_time_of_a_real_transcript() {
    assert_eq!(
        parse(TRANSCRIPT),
        Some(Apply {
            result: "OK".into(),
            finished: Some("2026-09-17T10:18:49Z".into()),
        })
    );
}

#[test]
fn reports_a_failed_apply_as_the_transcript_spelled_it() {
    let failed = TRANSCRIPT.replace("result:     OK", "result:     FAILED");
    assert_eq!(
        parse(&failed).unwrap().summary(),
        "FAILED at 2026-09-17T10:18:49Z"
    );
}

#[test]
fn a_transcript_with_no_result_line_is_not_an_apply_outcome() {
    assert_eq!(
        parse("chezmoi apply\nstarted:    2026-09-17T10:17:26Z\n"),
        None
    );
}

#[test]
fn a_transcript_that_never_finished_still_reports_its_result() {
    let unfinished = "exit_code:  1\nresult:     FAILED\n";
    assert_eq!(
        parse(unfinished).unwrap().summary(),
        "FAILED, no finish time recorded"
    );
}

#[test]
fn the_last_apply_in_an_appended_transcript_wins() {
    let two = format!("{TRANSCRIPT}{}", TRANSCRIPT.replace("10:18:49", "11:30:02"));
    assert_eq!(
        parse(&two).unwrap().finished.unwrap(),
        "2026-09-17T11:30:02Z"
    );
}
