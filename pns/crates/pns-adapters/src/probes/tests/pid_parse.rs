use super::*;

// --- parse_pids ---------------------------------------------------------

#[test]
fn every_decimal_id_is_kept_one_per_line() {
    assert_eq!(parse_pids("101\n2002\n"), vec!["101", "2002"]);
}

#[test]
fn a_line_that_is_not_a_plain_id_is_discarded_rather_than_passed_to_the_sampler() {
    assert_eq!(
        parse_pids("101\nnot-a-pid\n-5\n\n2002\n"),
        vec!["101", "2002"]
    );
}

#[test]
fn no_sessions_at_all_is_an_empty_list_and_never_an_error() {
    assert!(parse_pids("").is_empty());
}

#[test]
fn a_padded_pid_line_is_rejected_like_any_other_malformed_line() {
    // The bash reference validates the raw line; trimming first would
    // promote garbled output into a trusted process id.
    assert_eq!(parse_pids(" 101 \n2002\n"), vec!["2002"]);
}
