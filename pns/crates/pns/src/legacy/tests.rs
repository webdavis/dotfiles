/// One flag cannot contradict itself, so the pair that could is refused as two
/// retired spellings rather than as a combination that suppressed every
/// channel and still exited 0.
#[test]
fn each_retired_narrowing_flag_is_refused_with_exit_two() {
    for flag in ["--local-only", "--remote-only"] {
        let argv = [flag.to_owned()];
        assert_eq!(
            super::run(&argv, |_, _| panic!("a retired flag reached submission")),
            2
        );
    }
}

/// And a scope word none of the three is refused the same way, rather than
/// falling back to automatic and sending off the machine what the caller asked
/// to keep on it.
#[test]
fn a_scope_outside_the_three_words_is_refused_with_exit_two() {
    let argv = ["--scope".to_owned(), "local".to_owned()];
    assert_eq!(
        super::run(&argv, |_, _| panic!("an unknown scope reached submission")),
        2
    );
}

/// EVERY CALLER HEARS THE DELIVERY NOW, whether or not it asked: a page that
/// is nowhere is the one thing a synchronous producer has no other way to
/// learn, and the opt-in that used to gate the answer hid it from everyone who
/// did not know to type it.
#[test]
fn every_caller_hears_a_failed_delivery() {
    let argv = ["--producer".to_string(), "posture".to_string()];
    assert_eq!(super::run(&argv, |_, _| 1), 1);
}

/// And the path invents no failure: a delivery that landed still exits 0.
#[test]
fn a_delivery_that_landed_still_exits_zero() {
    let argv = ["--producer".to_string(), "posture".to_string()];
    assert_eq!(super::run(&argv, |_, _| 0), 0);
}

/// The retired opt-in is REFUSED rather than skipped, and the refusal says why
/// it is gone. Skipping it would leave a caller believing it still gates an
/// answer that is now unconditional.
#[test]
fn the_retired_require_delivery_flag_is_refused_with_exit_two() {
    let argv = [
        "--producer".to_string(),
        "posture".to_string(),
        "--require-delivery".to_string(),
    ];
    assert_eq!(
        super::run(&argv, |_, _| panic!("a retired flag reached submission")),
        2
    );
    let refusal = super::parse_args(argv.iter().cloned())
        .into_event()
        .unwrap_err();
    assert!(refusal.contains("always reports delivery"), "{refusal}");
}

/// The retired flag is REFUSED rather than skipped, and the refusal names its
/// replacement. Skipping it is what the lenient rule would do, and that sends
/// the event under the default producer with the sender's own name dropped.
#[test]
fn the_retired_agent_flag_is_refused_and_names_the_flag_that_replaced_it() {
    let argv = [
        "--agent".to_string(),
        "posture".to_string(),
        "--state".to_string(),
        "done".to_string(),
    ];
    assert_eq!(
        super::run(&argv, |_, _| panic!("a retired flag reached submission")),
        2
    );
}

/// A delivery class that is not a usable name is refused whole, and so is the
/// retired `--kind` spelling: either one delivered by guess would put a page
/// on the routine channel or a routine event on the one reserved for things
/// that need a human.
#[test]
fn an_unusable_delivery_class_is_refused_before_anything_is_delivered() {
    for word in ["", "bad\nclass", "--producer"] {
        let argv = ["--delivery-class".to_string(), word.to_string()];
        assert_eq!(
            super::run(&argv, |_, _| panic!(
                "an unusable delivery class reached submission"
            )),
            2,
            "--delivery-class {word:?}"
        );
    }
    let retired = ["--kind".to_string(), "health".to_string()];
    assert_eq!(
        super::run(&retired, |_, _| panic!("a retired flag reached submission")),
        2
    );
}

/// The two identifiers a JSON producer has always sent are flags now, and they
/// reach the same places: the id the submission is recorded under, and the
/// session the payload names.
#[test]
fn the_request_id_and_the_session_reach_the_submission_a_caller_named_them_for() {
    let argv = [
        "--producer",
        "posture",
        "--state",
        "done",
        "--request-id",
        "posture-occurrence-1",
        "--session",
        "s-2026-09-17-a",
    ]
    .map(str::to_owned);
    let mut seen = None;
    assert_eq!(
        super::run(&argv, |event, session| {
            seen = Some((event.request_id, session));
            0
        }),
        0
    );
    assert_eq!(
        seen,
        Some((
            "posture-occurrence-1".to_owned(),
            "s-2026-09-17-a".to_owned()
        ))
    );
}

/// NONE OF THEM IS REQUIRED. A caller that names neither still sends, and pns
/// mints the id it was not given.
#[test]
fn a_request_naming_neither_identifier_still_sends() {
    let argv = ["--producer", "posture", "--state", "done"].map(str::to_owned);
    let mut seen = None;
    assert_eq!(
        super::run(&argv, |event, session| {
            seen = Some((event.request_id, session));
            0
        }),
        0
    );
    assert_eq!(seen, Some((String::new(), String::new())));
}

/// Held to the identifier rules the envelope holds its JSON twin to, so one
/// spelling cannot carry a value the other refuses.
#[test]
fn an_identifier_the_envelope_would_refuse_is_refused_on_the_flag_path_too() {
    for (flag, value) in [
        ("--request-id", ""),
        ("--request-id", "has space"),
        ("--session", ""),
        ("--session", "a\nb"),
    ] {
        let argv = [
            "--producer".to_owned(),
            "posture".to_owned(),
            flag.to_owned(),
            value.to_owned(),
        ];
        assert_eq!(
            super::run(&argv, |_, _| panic!("{flag} {value:?} reached submission")),
            2,
            "{flag} {value:?}"
        );
    }
}

/// THE SAME DURATION ON BOTH PATHS, and a bare number on neither: `90` is
/// seconds to one reader and minutes to the next.
#[test]
fn elapsed_takes_a_duration_and_refuses_a_bare_number() {
    for (value, detail) in [("90s", "90s"), ("2m", "120s"), ("5m", "300s")] {
        let argv = [
            "--producer".to_owned(),
            "nvim".to_owned(),
            "--state".to_owned(),
            "done".to_owned(),
            "--elapsed".to_owned(),
            value.to_owned(),
        ];
        let mut seen = None;
        assert_eq!(
            super::run(&argv, |event, _| {
                seen = Some(event.detail);
                0
            }),
            0,
            "{value}"
        );
        assert_eq!(seen.as_deref(), Some(detail), "{value}");
    }
    for value in ["90", "0", "", "90 s", "ninety"] {
        let argv = [
            "--producer".to_owned(),
            "nvim".to_owned(),
            "--elapsed".to_owned(),
            value.to_owned(),
        ];
        assert_eq!(
            super::run(&argv, |_, _| panic!(
                "--elapsed {value:?} reached submission"
            )),
            2,
            "--elapsed {value:?}"
        );
    }
}
