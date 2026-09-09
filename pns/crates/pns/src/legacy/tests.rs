#[test]
fn both_narrowing_flags_plan_nothing_at_all() {
    for flags in [
        ["--local-only", "--remote-only"],
        ["--remote-only", "--local-only"],
    ] {
        let argv = flags.map(str::to_owned);
        assert_eq!(
            super::run(&argv, |_| panic!("invalid scope reached submission")),
            0
        );
    }
}

/// DECISION 0010 IS THE DEFAULT: a notification never fails the work it reports
/// on, so a caller that did not ask hears 0 whatever the delivery answered.
#[test]
fn a_caller_that_did_not_ask_never_hears_a_failed_delivery() {
    let argv = ["--agent".to_string(), "posture".to_string()];
    assert_eq!(super::run(&argv, |_| 1), 0);
}

/// And a caller that DID ask hears it, which is the whole of what a producer
/// such as posture can read: it has no other way to learn that the page it just
/// sent is nowhere.
#[test]
fn a_caller_that_asked_hears_a_failed_delivery() {
    let argv = [
        "--agent".to_string(),
        "posture".to_string(),
        "--require-delivery".to_string(),
    ];
    assert_eq!(super::run(&argv, |_| 1), 1);
}

/// The flag says whether the caller HEARS the answer, never what the answer is:
/// a delivery that landed exits 0 under it, the same as without it.
#[test]
fn asking_for_the_answer_does_not_invent_a_failure() {
    let argv = [
        "--agent".to_string(),
        "posture".to_string(),
        "--require-delivery".to_string(),
    ];
    assert_eq!(super::run(&argv, |_| 0), 0);
}
