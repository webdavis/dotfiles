use super::*;

#[test]
fn quota_auto_resume_stale_arms_the_needs_marker_for_its_own_session() {
    let sandbox = Sandbox::new("quota-stale-arms-marker");
    sandbox.write_config(&format!("{}{LAMPS_ON}", nag_config(300)));
    counted_channels(&sandbox);

    let output = hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "quota",
        &quota_payload("s1", "quota_auto_resume_stale", "press enter to continue"),
    );

    assert!(output.status.success());
    assert_eq!(
        deliveries(&sandbox, "hermes"),
        1,
        "the positive control: the observation itself delivered"
    );
    assert_eq!(
        waiting_sessions(&sandbox),
        vec!["s1".to_string()],
        "a stale wait arms its own session's needs marker"
    );
}

#[test]
fn a_stale_wait_arms_the_needs_marker_before_the_card_is_delivered() {
    // THE ORDER IS THE RACE. The declaration is `async: true`, so this hook
    // runs beside the session rather than in front of it, and the operator
    // presses Enter on a screen that is already telling them to. Arming
    // AFTER the delivery plan means a whole plan of network legs runs first,
    // and an Enter inside that window clears nothing (there is no marker yet)
    // and then gets a marker published behind it: a blocked lamp for a session
    // that is already working again, held until its turn's own Stop.
    // Arming first cannot close the race, which is the harness's to close,
    // but it shrinks the window from a delivery plan to one file write.
    let sandbox = Sandbox::new("quota-stale-arms-before-delivery");
    sandbox.write_config(&format!("{}{LAMPS_ON}", nag_config(300)));
    counted_channels(&sandbox);
    // The delivery itself reports what the state directory held WHILE it ran.
    sandbox.stub_channel(
        "hermes",
        &format!(
            "ls \"{s}/state/lights-blocked\" >\"{s}/waiting-at-delivery\" 2>&1; \
             printf 'x' >>\"{s}/hermes.count\"; cat >\"{s}/hermes.event\"",
            s = sandbox.display()
        ),
    );

    let output = hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "quota",
        &quota_payload("s1", "quota_auto_resume_stale", "press enter to continue"),
    );

    assert!(output.status.success());
    assert_eq!(
        deliveries(&sandbox, "hermes"),
        1,
        "the positive control: the observation itself delivered"
    );
    assert_eq!(
        std::fs::read_to_string(sandbox.path("waiting-at-delivery")).unwrap_or_default(),
        "s1\n",
        "the marker is already published when the first leg runs"
    );
}

#[test]
fn quota_auto_resume_fired_and_disabled_arm_no_needs_marker() {
    // THE MIRROR OF THE TEST ABOVE: proving stale arms the marker says
    // nothing about whether the other two also do, and a mutant that arms it
    // for every type would still pass a test that only checks stale.
    for notification_type in ["quota_auto_resume_fired", "quota_auto_resume_disabled"] {
        let sandbox = Sandbox::new(&format!("quota-no-arm-{notification_type}"));
        sandbox.write_config(&format!("{}{LAMPS_ON}", nag_config(300)));
        counted_channels(&sandbox);

        let output = hook_with(
            with_state_dir(&sandbox),
            &sandbox,
            "quota",
            &quota_payload("s1", notification_type, "m"),
        );

        assert!(output.status.success(), "{notification_type}");
        assert_eq!(
            deliveries(&sandbox, "hermes"),
            1,
            "{notification_type}: the positive control fired"
        );
        assert!(
            waiting_sessions(&sandbox).is_empty(),
            "{notification_type}: must arm no needs marker"
        );
    }
}

#[test]
fn the_prompt_hook_clears_a_stale_quota_marker() {
    // Q3'S OWN CLOSE, THE FAST PATH. Whatever Claude Code's own continuation
    // prompt does, the operator typing anything in that session ends the wait
    // the way any other prompt does. The guarantee that does not depend on a
    // prompt at all is the test below.
    let sandbox = Sandbox::new("quota-stale-cleared-by-prompt");
    sandbox.write_config(&format!("{}{LAMPS_ON}", nag_config(300)));
    counted_channels(&sandbox);

    let armed = hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "quota",
        &quota_payload("s1", "quota_auto_resume_stale", "press enter to continue"),
    );
    assert!(armed.status.success());
    assert_eq!(
        waiting_sessions(&sandbox),
        vec!["s1".to_string()],
        "the precondition: a stale wait armed the marker"
    );

    let prompted = hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "prompt",
        r#"{"session_id":"s1"}"#,
    );

    assert!(prompted.status.success());
    assert!(
        waiting_sessions(&sandbox).is_empty(),
        "the operator's continuation clears the marker the way any other prompt does"
    );
}

#[test]
fn a_stale_quota_marker_clears_at_the_turns_stop_without_any_prompt_hook() {
    // Q3'S GUARANTEE, AND WHY IT IS SEPARATE FROM THE TEST ABOVE. Claude Code
    // continues a wait by sending Claude a fixed prompt of its own, and its
    // reference does not say whether that internal prompt reaches the
    // `UserPromptSubmit` hook. If it does not, the marker armed here would be
    // cleared by nothing at all unless something else ends it. Something else
    // does: every event from that session except the four that start a wait
    // ends one, so the continued turn's own Stop clears it with no prompt hook
    // in the sequence at all.
    let sandbox = Sandbox::new("quota-stale-cleared-by-stop");
    sandbox.write_config(&format!("{}{LAMPS_ON}", nag_config(300)));
    counted_channels(&sandbox);

    let armed = hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "quota",
        &quota_payload("s1", "quota_auto_resume_stale", "press enter to continue"),
    );
    assert!(armed.status.success());
    assert_eq!(
        waiting_sessions(&sandbox),
        vec!["s1".to_string()],
        "the precondition: a stale wait armed the marker"
    );

    let stopped = hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "stop",
        r#"{"session_id":"s1","cwd":"/a/dotfiles"}"#,
    );

    assert!(stopped.status.success());
    assert!(
        waiting_sessions(&sandbox).is_empty(),
        "the continued turn ending clears the marker with no prompt hook in the sequence"
    );
}
