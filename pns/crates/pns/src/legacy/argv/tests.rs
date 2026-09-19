use super::parse_args;

fn args(tokens: &[&str]) -> (super::EventArgs, Vec<String>) {
    let parsed = parse_args(tokens.iter().map(|t| t.to_string()));
    let warnings = parsed.warnings.clone();
    (parsed.into_event().ok().flatten().unwrap(), warnings)
}

#[test]
fn every_value_flag_lands_in_its_field() {
    let (parsed, warnings) = args(&[
        "--producer",
        "claude",
        "--state",
        "done",
        "--project",
        "dotfiles",
        "--branch",
        "main",
        "--detail",
        "a summary",
        "--pane",
        "wW:p21",
        "--scope",
        "local_only",
    ]);
    assert_eq!(parsed.agent, "claude");
    assert_eq!(parsed.state, "done");
    assert_eq!(parsed.project, "dotfiles");
    assert_eq!(parsed.branch, "main");
    assert_eq!(parsed.detail, "a summary");
    assert_eq!(parsed.pane, "wW:p21");
    assert_eq!(parsed.scope, super::DeliveryScope::LocalOnly);
    assert!(warnings.is_empty());
}

#[test]
fn the_route_flag_names_a_route_and_is_protected_like_every_value_flag() {
    let (parsed, warnings) = args(&["--route", "log", "--producer", "brew"]);
    assert_eq!(parsed.channel, "log");
    assert_eq!(parsed.agent, "brew");
    assert!(warnings.is_empty());
    // And it is never eaten as another flag's value.
    let (parsed, warnings) = args(&["--detail", "--route", "log"]);
    assert_eq!(parsed.detail, "");
    assert_eq!(parsed.channel, "log");
    assert_eq!(warnings.len(), 1);
}

#[test]
fn the_retired_channel_flag_takes_its_value_with_it_and_carries_the_refusal() {
    // With a value present, it goes with the retired flag rather than
    // leaking through to any later processing.
    let with_value = parse_args(["--channel", "priority", "--state", "done"].map(str::to_owned));
    assert!(with_value.warnings.is_empty());
    assert_eq!(with_value.event.state, "done");
    assert!(matches!(
        with_value.into_event(),
        Err(message) if message == "--channel was replaced by --route"
    ));

    // With no value present, the next real flag is never swallowed as one.
    let without_value = parse_args(["--channel", "--state", "done"].map(str::to_owned));
    assert!(without_value.warnings.is_empty());
    assert_eq!(without_value.event.state, "done");
    assert!(matches!(
        without_value.into_event(),
        Err(message) if message == "--channel was replaced by --route"
    ));
}

#[test]
fn the_retired_agent_flag_takes_its_value_with_it_and_carries_the_refusal() {
    let parsed = parse_args(["--agent", "codex", "--state", "done"].map(str::to_owned));
    // Its value never becomes another field's, and the refusal is the one
    // thing the parse hands on.
    assert!(parsed.warnings.is_empty());
    assert!(matches!(
        parsed.into_event(),
        Err(message) if message == "--agent was replaced by --producer"
    ));
}

/// The pair `--scope` replaced could contradict itself, so a refusal existed
/// for being given both. One flag cannot, so the refusal is gone and each old
/// spelling is refused naming the flag that replaced it.
#[test]
fn each_retired_narrowing_flag_is_refused_and_names_the_scope_flag() {
    for flag in ["--local-only", "--remote-only"] {
        let parsed = parse_args([flag, "--state", "done"].map(str::to_owned));
        assert!(parsed.warnings.is_empty());
        assert!(
            matches!(
                parsed.into_event(),
                Err(message) if message == format!("{flag} was replaced by --scope")
            ),
            "{flag} was not refused"
        );
    }
}

#[test]
fn a_scope_outside_the_three_words_refuses_the_event_and_names_them() {
    for word in ["local", "LocalOnly", "local-only", "both", ""] {
        let parsed = parse_args(["--producer", "uu", "--scope", word].map(str::to_owned));
        assert!(
            matches!(
                parsed.into_event(),
                Err(message)
                    if message == "--scope requires one of: automatic, local_only, remote_only"
            ),
            "{word} was not refused"
        );
    }
    for (word, expected) in [
        ("automatic", super::DeliveryScope::Automatic),
        ("local_only", super::DeliveryScope::LocalOnly),
        ("remote_only", super::DeliveryScope::RemoteOnly),
    ] {
        let parsed = parse_args(["--producer", "uu", "--scope", word].map(str::to_owned));
        let event = parsed.into_event().ok().flatten().expect("a stated scope");
        assert_eq!(event.scope, expected);
    }
    // Every word the domain accepts is one this flag accepts.
    assert_eq!(super::DeliveryScope::WORDS.len(), 3);
}

/// A trailing `--scope` with no value at all refuses the same way an unknown
/// word does, rather than delivering under the default it never asked for.
#[test]
fn a_trailing_scope_with_no_value_refuses_like_an_unknown_one() {
    let parsed = parse_args(["--producer", "uu", "--scope"].map(str::to_owned));
    assert!(parsed.warnings.is_empty());
    assert!(matches!(
        parsed.into_event(),
        Err(message) if message == "--scope requires one of: automatic, local_only, remote_only"
    ));
}

/// With no `--scope` at all the event is automatic, which is what every hook,
/// the shell notifier and the daemon pass.
#[test]
fn no_scope_flag_leaves_the_event_automatic() {
    let (parsed, warnings) = args(&["--producer", "claude"]);
    assert_eq!(parsed.scope, super::DeliveryScope::Automatic);
    assert!(warnings.is_empty());
}

#[test]
fn a_failed_health_class_pages_and_a_session_class_keeps_the_default_route() {
    // A PRODUCER NAMES WHAT ITS EVENT IS; the route it lands on is pns's to
    // decide, and the route's NAME is the operator's, so the parse carries
    // the delivery class and nothing resolves a route here.
    let (parsed, warnings) = args(&[
        "--delivery-class",
        "health",
        "--state",
        "failed",
        "--producer",
        "upgrades",
    ]);
    // THE SAME WORD THE JSON PATH CARRIES, and it earns the same route:
    // `event_flow::submit::mapping` pins the JSON half of this pair.
    assert_eq!(parsed.delivery_class, "health");
    assert_eq!(parsed.channel, "", "the parse pinned a route name");
    assert_eq!(parsed.routed(Some("sirens")).channel, "sirens");
    assert!(warnings.is_empty());

    // A class whose table names no route of its own keeps the default route,
    // and so does a message naming no class at all: which classes route where
    // is `[delivery_class.<name>]`, never a word written here.
    for argv in [
        vec!["--producer", "claude"],
        vec!["--delivery-class", "agent"],
        vec!["--delivery-class", "security"],
    ] {
        let (parsed, _) = args(&argv);
        assert_eq!(
            parsed.routed(Some("")).channel,
            "",
            "{argv:?} must keep the default route"
        );
    }
}

#[test]
fn a_named_route_beats_the_delivery_class_in_either_order() {
    for argv in [
        vec!["--delivery-class", "health", "--route", "log"],
        vec!["--route", "log", "--delivery-class", "health"],
    ] {
        let (parsed, _) = args(&argv);
        assert_eq!(
            parsed.routed(Some("sirens")).channel,
            "log",
            "{argv:?}: a producer that said where already answered the question"
        );
    }
}

/// The flag `--delivery-class` replaced. It is refused rather than skipped,
/// and the refusal names its replacement.
#[test]
fn the_retired_kind_flag_takes_its_value_with_it_and_names_the_flag_that_replaced_it() {
    let parsed = parse_args(["--kind", "health", "--state", "done"].map(str::to_owned));
    assert!(parsed.warnings.is_empty());
    assert!(matches!(
        parsed.into_event(),
        Err(message) if message == "--kind was replaced by --delivery-class"
    ));
}

#[test]
fn a_recognized_flag_is_never_consumed_as_a_value() {
    // `--pane --scope`: eating the narrowing flag as the pane value would
    // deliver an event the caller asked to keep local.
    let (parsed, warnings) = args(&["--pane", "--scope", "local_only", "--producer", "claude"]);
    assert_eq!(parsed.pane, "");
    assert_eq!(
        parsed.scope,
        super::DeliveryScope::LocalOnly,
        "the narrowing flag must still apply"
    );
    assert_eq!(parsed.agent, "claude");
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("--pane"), "the warning names the flag");
}

#[test]
fn the_long_running_flag_is_retired_and_protected_from_being_eaten_like_every_other_one() {
    // It used to be handled but left out of the predicate, so `--detail
    // --long-running` swallowed it as the detail text. Now it is retired
    // outright: pns derives the tier from `--elapsed` alone.
    let parsed = parse_args(["--detail", "--long-running"].map(str::to_owned));
    assert_eq!(parsed.event.detail, "");
    assert_eq!(parsed.warnings.len(), 1);
    assert!(
        parsed.warnings[0].contains("--detail"),
        "the warning names the flag"
    );
    assert!(matches!(
        parsed.into_event(),
        Err(message) if message == "--long-running was replaced by --elapsed"
    ));
}

#[test]
fn a_trailing_value_flag_is_warned_and_ignored() {
    let (parsed, warnings) = args(&["--producer", "claude", "--detail"]);
    assert_eq!(parsed.agent, "claude");
    assert_eq!(parsed.detail, "");
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("--detail"));
}

#[test]
fn an_unrecognized_token_is_still_taken_as_a_value() {
    // The bash deliberately kept this leniency: only RECOGNIZED flags are
    // protected from being eaten.
    let (parsed, warnings) = args(&["--producer", "--bogus"]);
    assert_eq!(parsed.agent, "--bogus");
    assert!(warnings.is_empty());
}

#[test]
fn unknown_arguments_are_skipped_in_silence() {
    let (parsed, warnings) = args(&["stray", "--producer", "claude", "--wat"]);
    assert_eq!(parsed.agent, "claude");
    assert!(warnings.is_empty());
}

#[test]
fn help_in_flag_position_is_recognized_wherever_it_sits() {
    for tokens in [
        &["--help"][..],
        &["-h"][..],
        &["--producer", "claude", "--help"][..],
        &["--scope", "local_only", "--help"][..],
        // A RETIRED FLAG TAKES NO VALUE HERE, so it never eats the help that
        // follows it the way `--channel --help` would.
        &["--local-only", "--help"][..],
        &["stray", "--help"][..],
    ] {
        let parsed = parse_args(tokens.iter().map(|token| token.to_string()));
        assert!(parsed.help, "{tokens:?} should set help");
    }
}

#[test]
fn help_in_value_position_is_still_just_a_value() {
    // H-F, PINNED: `--help` sitting where a flag's value belongs is a
    // value, under the same leniency `an_unrecognized_token_is_still_taken_as_a_value`
    // pins for `--bogus`. Adding `--help` to `is_producer_flag` would flip
    // this into a warn-and-drop, which is the wrong fix.
    let parsed = parse_args(["--producer", "--help", "--state", "done"].map(str::to_owned));
    assert_eq!(parsed.event.agent, "--help");
    assert!(!parsed.help);
    assert!(parsed.warnings.is_empty());

    // The state is still read as a value rather than as help, and the closed
    // set is what refuses it afterwards.
    let parsed = parse_args(["--producer", "claude", "--state", "--help"].map(str::to_owned));
    assert_eq!(parsed.event.state, "--help");
    assert!(!parsed.help);
    assert!(matches!(
        parsed.into_event(),
        Err(message)
            if message == "--state requires one of: done, failed, blocked, resolved, observation, progress"
    ));
}

#[test]
fn a_state_outside_the_six_words_refuses_the_event_and_names_them() {
    for word in [
        "succeeded",
        "needs_attention",
        "first-install-failed",
        "Done",
        "",
    ] {
        let parsed = parse_args(["--producer", "uu", "--state", word].map(str::to_owned));
        assert!(
            matches!(
                parsed.into_event(),
                Err(message)
                    if message == "--state requires one of: done, failed, blocked, resolved, observation, progress"
            ),
            "{word} was not refused"
        );
    }
    for word in super::State::WORDS {
        let parsed = parse_args(["--producer", "uu", "--state", word].map(str::to_owned));
        let event = parsed.into_event().ok().flatten().expect("a stated word");
        assert_eq!(event.state, *word);
    }
}

/// A trailing `--state` with no value at all refuses the same way `--state
/// ""` does, rather than warning and delivering an event with no state.
#[test]
fn a_trailing_state_with_no_value_refuses_like_an_empty_one() {
    let parsed = parse_args(["--producer", "uu", "--state"].map(str::to_owned));
    assert!(parsed.warnings.is_empty());
    assert!(matches!(
        parsed.into_event(),
        Err(message)
            if message == "--state requires one of: done, failed, blocked, resolved, observation, progress"
    ));
}

#[test]
fn the_last_value_wins_for_every_producer_field() {
    let mut tokens = Vec::new();
    for flag in [
        "--producer",
        "--project",
        "--branch",
        "--detail",
        "--pane",
        "--route",
    ] {
        tokens.extend([flag, "first", flag, "last"]);
    }
    // The state is one of six words in either position, so its own pair is
    // two legal words rather than the placeholder the others use.
    tokens.extend(["--state", "done", "--state", "failed"]);
    let (event, warnings) = args(&tokens);
    assert_eq!(
        [
            event.agent,
            event.project,
            event.branch,
            event.detail,
            event.pane,
            event.channel
        ],
        ["last"; 6].map(str::to_owned),
    );
    assert_eq!(event.state, "failed");
    assert!(warnings.is_empty());
}

#[test]
fn the_last_reminder_switch_argv_named_is_the_one_that_answers() {
    // A WRAPPER APPENDS. A harness declaration that already carries a switch
    // and a caller that adds its own must not leave the first one in charge,
    // which is what a first-one-wins scan would do.
    let switches = |tokens: &[&str]| {
        super::remind_switch(&tokens.iter().map(|t| t.to_string()).collect::<Vec<_>>())
    };
    assert_eq!(switches(&[]), Ok(None));
    assert_eq!(switches(&["--remind"]), Ok(Some(super::Remind::Configured)));
    assert_eq!(
        switches(&["--remind", "--no-remind"]),
        Ok(Some(super::Remind::Off))
    );
    assert_eq!(
        switches(&["--no-remind", "--remind=90s"]),
        Ok(Some(super::Remind::After(90)))
    );
    // A WORD THAT MERELY STARTS THE SAME IS NOT THE FLAG.
    assert_eq!(switches(&["--reminder", "--remind-me"]), Ok(None));
    assert!(
        switches(&["--remind=90"]).is_err(),
        "a bare number is not a duration"
    );
}
