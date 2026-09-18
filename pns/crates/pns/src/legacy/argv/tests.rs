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
        "--local-only",
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
    let parsed = parse_args(["--channel", "priority", "--state", "done"].map(str::to_owned));
    // Its value goes with it: a lenient skip would read `priority` as a stray
    // word and post to the default route while looking like it worked.
    assert!(parsed.warnings.is_empty());
    assert!(matches!(
        parsed.into_event(),
        Err(super::Refusal::Value(message)) if message == "--channel was replaced by --route"
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
        Err(super::Refusal::Value(message)) if message == "--agent was replaced by --producer"
    ));
}

#[test]
fn a_failed_health_kind_pages_and_an_agent_kind_keeps_the_default_route() {
    // A PRODUCER NAMES WHAT ITS EVENT IS; the route it lands on is pns's to
    // decide, and the route's NAME is the operator's, so the parse carries
    // the kind and nothing resolves a route here.
    let routes = pns_domain::routes::Routes::named("logbook", "sirens");
    let (parsed, warnings) = args(&[
        "--kind",
        "health",
        "--state",
        "failed",
        "--producer",
        "upgrades",
    ]);
    assert_eq!(parsed.kind, pns_domain::routes::Kind::Health);
    assert_eq!(parsed.channel, "", "the parse pinned a route name");
    assert_eq!(parsed.routed(&routes).channel, "sirens");
    assert!(warnings.is_empty());

    // The default kind is the behavior every producer already had: an empty
    // route, which the hermes target reads as the default one.
    for argv in [vec!["--producer", "claude"], vec!["--kind", "agent"]] {
        let (parsed, _) = args(&argv);
        assert_eq!(
            parsed.routed(&routes).channel,
            "",
            "{argv:?} must keep the default route"
        );
    }
}

#[test]
fn a_named_route_beats_the_kind_in_either_order() {
    let routes = pns_domain::routes::Routes::named("logbook", "sirens");
    for argv in [
        vec!["--kind", "health", "--route", "log"],
        vec!["--route", "log", "--kind", "health"],
    ] {
        let (parsed, _) = args(&argv);
        assert_eq!(
            parsed.routed(&routes).channel,
            "log",
            "{argv:?}: a producer that said where already answered the question"
        );
    }
}

#[test]
fn a_recognized_flag_is_never_consumed_as_a_value() {
    // `--pane --local-only`: eating the narrowing flag as the pane value
    // would deliver an event the caller asked to keep local.
    let (parsed, warnings) = args(&["--pane", "--local-only", "--producer", "claude"]);
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
fn the_long_running_flag_is_protected_from_being_eaten_like_every_other_one() {
    // It was handled but left out of the predicate, so `--detail
    // --long-running` swallowed it as the detail text: the notification
    // carried a flag name as its summary AND lost the tier that decides
    // the lights, both in silence.
    let (parsed, warnings) = args(&["--detail", "--long-running"]);
    assert_eq!(parsed.detail, "");
    assert!(parsed.long_running, "the tier must still apply");
    assert_eq!(warnings.len(), 1);
    assert!(
        warnings[0].contains("--detail"),
        "the warning names the flag"
    );
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

    let parsed = parse_args(["--producer", "claude", "--state", "--help"].map(str::to_owned));
    assert_eq!(parsed.event.state, "--help");
    assert!(!parsed.help);
}

#[test]
fn the_last_value_wins_for_every_producer_field() {
    let mut tokens = Vec::new();
    for flag in [
        "--producer",
        "--state",
        "--project",
        "--branch",
        "--detail",
        "--pane",
        "--route",
    ] {
        tokens.extend([flag, "first", flag, "last"]);
    }
    let (event, warnings) = args(&tokens);
    assert_eq!(
        [
            event.agent,
            event.state,
            event.project,
            event.branch,
            event.detail,
            event.pane,
            event.channel
        ],
        ["last"; 7].map(str::to_owned),
    );
    assert!(warnings.is_empty());
}
