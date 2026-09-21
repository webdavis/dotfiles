use super::*;

/// A clock that states the moment rather than reading one.
fn stamp(epoch: u64) -> Option<String> {
    Some(format!("2026-09-19T{:02}:00:00-04:00", epoch))
}

fn words(argv: &[&str]) -> Vec<String> {
    argv.iter().map(|word| (*word).to_string()).collect()
}

#[test]
fn the_window_is_substituted_into_the_words_the_operator_wrote() {
    // THE PLACEHOLDER IS REPLACED INSIDE A WORD, not only as a whole one, so
    // `--search=updated:>={since}` works the way an operator writes it.
    let read = run_source(
        &words(&["echo", "from={since}", "--until={until}"]),
        Some(6),
        Some(12),
        stamp,
    );
    assert_eq!(
        read.rows(),
        ["from=2026-09-19T06:00:00-04:00 --until=2026-09-19T12:00:00-04:00"]
    );
}

#[test]
fn a_command_with_no_window_keeps_its_words_unchanged() {
    let read = run_source(&words(&["echo", "{since}"]), None, None, stamp);
    assert_eq!(read.rows(), ["{since}"]);
}

#[test]
fn one_row_per_line_and_a_blank_line_is_not_a_row() {
    let read = run_source(&words(&["printf", "one\\n\\ntwo\\n"]), None, None, stamp);
    assert_eq!(read.rows(), ["one", "two"]);
}

#[test]
fn a_command_that_exits_non_zero_reports_its_code_rather_than_an_empty_section() {
    assert_eq!(
        run_source(&words(&["sh", "-c", "exit 3"]), None, None, stamp),
        Sourcing::Failed(3)
    );
}

#[test]
fn a_command_that_is_not_installed_is_unavailable_rather_than_failed() {
    assert_eq!(
        run_source(&words(&["pns-no-such-source-command"]), None, None, stamp),
        Sourcing::Unavailable
    );
}

#[test]
fn a_window_the_clock_cannot_state_never_runs_the_command_with_the_placeholder_in_it() {
    // THE FENCE: `{since}` reaching a program verbatim is a search for a
    // literal brace, which reports a window nobody asked for.
    assert_eq!(
        run_source(&words(&["echo", "{since}"]), Some(6), Some(12), |_| None),
        Sourcing::Unavailable
    );
}
