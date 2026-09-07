use super::*;

// --- observation mode: the quota notification arm --------------------------
//
// D7's cheapest shape: the ONE `Notification` matcher this binary recognises
// is the three quota auto-resume types, routed through `Attempt::Observation`
// exactly like the model-switch arm above. `quota_auto_resume_stale` is the
// one exception (Q3): the interactive-mode reference documents that after a
// long sleep the session stops and reads `press enter to continue`, which is a
// wait on the operator, so `stale` alone also arms the needs marker directly.
// `fired` and `disabled` stay marker-neutral because neither reports a session
// waiting on anybody. What CLEARS the marker is pinned by two tests, not by
// one: the reference does not say whether Claude Code's own continuation
// prompt reaches the `UserPromptSubmit` hook, so the prompt hook is tested as
// the fast path and the turn's Stop as the guarantee that holds without it.

pub(crate) const QUOTA_TYPES: [&str; 3] = [
    "quota_auto_resume_fired",
    "quota_auto_resume_stale",
    "quota_auto_resume_disabled",
];

pub(crate) fn quota_payload(session: &str, notification_type: &str, message: &str) -> String {
    format!(
        r#"{{"session_id":"{session}","cwd":"/a/dotfiles","hook_event_name":"Notification","notification_type":"{notification_type}","message":"{message}"}}"#
    )
}

#[test]
fn quota_auto_resume_fired_delivers_one_card_naming_itself() {
    let sandbox = Sandbox::new("quota-fired-card");
    sandbox.write_config(&nag_config(300));
    counted_channels(&sandbox);

    let output = hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "quota",
        &quota_payload("s1", "quota_auto_resume_fired", "continuing automatically"),
    );

    assert!(output.status.success());
    assert_eq!(deliveries(&sandbox, "hermes"), 1, "exactly one card");
    let event = sandbox.event("hermes");
    assert_eq!(event["state"], "quota");
    assert_eq!(
        event["detail"],
        "quota auto-resume fired: continuing automatically"
    );
}

#[test]
fn quota_auto_resume_stale_delivers_one_card_naming_itself() {
    let sandbox = Sandbox::new("quota-stale-card");
    sandbox.write_config(&nag_config(300));
    counted_channels(&sandbox);

    let output = hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "quota",
        &quota_payload("s1", "quota_auto_resume_stale", "press enter to continue"),
    );

    assert!(output.status.success());
    assert_eq!(deliveries(&sandbox, "hermes"), 1, "exactly one card");
    let event = sandbox.event("hermes");
    assert_eq!(event["state"], "quota");
    assert_eq!(
        event["detail"],
        "quota auto-resume stale: press enter to continue"
    );
}

#[test]
fn quota_auto_resume_disabled_delivers_one_card_naming_itself() {
    let sandbox = Sandbox::new("quota-disabled-card");
    sandbox.write_config(&nag_config(300));
    counted_channels(&sandbox);

    let output = hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "quota",
        &quota_payload("s1", "quota_auto_resume_disabled", "turned off"),
    );

    assert!(output.status.success());
    assert_eq!(deliveries(&sandbox, "hermes"), 1, "exactly one card");
    let event = sandbox.event("hermes");
    assert_eq!(event["state"], "quota");
    assert_eq!(event["detail"], "quota auto-resume disabled: turned off");
}

#[test]
fn a_quota_notification_carrying_no_message_still_names_what_happened() {
    // A `Notification` payload whose `message` is absent, empty, or not a
    // string at all parses to the empty string, and the card must then be the
    // label alone rather than a label with a dangling separator after it.
    for message in [r#""message":"""#, r#""message":42"#, r#""unrelated":1"#] {
        let sandbox = Sandbox::new(&format!("quota-no-message-{}", message.len()));
        sandbox.write_config(&nag_config(300));
        counted_channels(&sandbox);

        let output = hook_with(
            with_state_dir(&sandbox),
            &sandbox,
            "quota",
            &format!(
                r#"{{"session_id":"s1","cwd":"/a/dotfiles","hook_event_name":"Notification","notification_type":"quota_auto_resume_fired",{message}}}"#
            ),
        );

        assert!(output.status.success(), "{message}");
        assert_eq!(deliveries(&sandbox, "hermes"), 1, "{message}: one card");
        assert_eq!(
            sandbox.event("hermes")["detail"],
            "quota auto-resume fired",
            "{message}: the label alone, with nothing trailing it"
        );
    }
}

#[test]
fn an_unrecognised_notification_type_delivers_nothing() {
    let sandbox = Sandbox::new("quota-unmatched-type");
    sandbox.write_config(&nag_config(300));
    counted_channels(&sandbox);

    // THE CONTROL: a matched type in this same sandbox proves the writer is
    // reachable, so the zero counts below are a decision, never a broken
    // harness.
    let control = hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "quota",
        &quota_payload("s1", "quota_auto_resume_fired", "continuing"),
    );
    assert!(control.status.success());
    assert_eq!(deliveries(&sandbox, "hermes"), 1, "the control fired");

    // A wildcard matcher would duplicate every one of these against the
    // lifecycle hooks that already cover it (D7's whole reason to exist), and
    // the two deferred D7 types are named explicitly among them.
    // AND THE NEAR MISSES, which is what makes this an EXACT allowlist rather
    // than a prefix one. The declaration's matcher is exact by the hooks
    // reference's own rule (letters, digits, `_`, `-`, spaces, `,` and `|`
    // only), but this binary keeps its own second allowlist, and a `quota`
    // arm widened to `starts_with("quota_auto_resume_")` passed every one of
    // these tests while none of the unrelated types below could reach it.
    for notification_type in [
        "permission_prompt",
        "idle_prompt",
        "auth_success",
        "elicitation_dialog",
        "elicitation_response",
        "agent_needs_input",
        "agent_completed",
        "",
        "quota_auto_resume_",
        "quota_auto_resume_paused",
        "quota_auto_resume_firedly",
        "quota_auto_resume_stale_again",
        "pre_quota_auto_resume_fired",
    ] {
        let output = hook_with(
            with_state_dir(&sandbox),
            &sandbox,
            "quota",
            &quota_payload("s2", notification_type, "something"),
        );
        assert!(output.status.success(), "{notification_type:?}");
        assert_eq!(
            deliveries(&sandbox, "hermes"),
            1,
            "{notification_type:?}: no card"
        );
    }
}

#[test]
fn every_quota_type_is_logged_as_an_observation_with_no_nag() {
    for notification_type in QUOTA_TYPES {
        let sandbox = Sandbox::new(&format!("quota-nag-{notification_type}"));
        sandbox.write_config(&nag_config(300));
        counted_channels(&sandbox);

        let output = hook_with(
            with_state_dir(&sandbox),
            &sandbox,
            "quota",
            &quota_payload("s1", notification_type, "m"),
        );
        assert!(output.status.success(), "{notification_type}");

        let recorded =
            std::fs::read_to_string(sandbox.path("state/decisions")).expect("the decision ring");
        let lines: Vec<&str> = recorded.lines().collect();
        assert_eq!(
            lines.len(),
            1,
            "{notification_type}: one event, one line: {recorded:?}"
        );
        assert!(
            lines[0].contains(" claude/quota "),
            "{notification_type}: names the harness and the state: {recorded:?}"
        );
        assert!(
            lines[0].contains(" nag=no "),
            "{notification_type}: an observation is logged with no nag: {recorded:?}"
        );
    }
}
