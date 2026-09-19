use super::*;

// --- the sandbox network approval dialog -------------------------------------
//
// A sandboxed command asking to reach the network gets no `PermissionRequest`
// at all: `SandboxNetworkPrompts` calls the dialog host directly, and the one
// hook-visible trace is a `Notification` whose registry entry states no type,
// which the host defaults to `permission_prompt`. The `Notification` matcher
// matches that type, so no matcher separates this dialog from an ordinary tool
// approval, and the payload names neither the host nor the port. The alert is
// therefore APPROXIMATE, which is a property of the platform rather than a
// defect here, and the Rust-side allowlist below is what keeps every other
// permission prompt silent.

/// The dialog host's own notification, as it reaches a hook.
fn permission_prompt(session: &str, message: &str) -> String {
    format!(
        r#"{{"session_id":"{session}","cwd":"/a/dotfiles","hook_event_name":"Notification","notification_type":"permission_prompt","message":"{message}"}}"#
    )
}

#[test]
fn a_sandboxed_command_asking_for_the_network_cards_the_operator_and_arms_the_wait() {
    let sandbox = Sandbox::new("sandbox-network-cards");
    sandbox.write_config(&format!("{}{}", remind_config(300), LAMPS_ON));
    counted_channels(&sandbox);

    let output = hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "waiting",
        &permission_prompt("s1", "A sandboxed command needs network access"),
    );

    assert_eq!(output.status.code(), Some(0));
    let event = sandbox.event("hermes");
    assert_eq!(
        event["state"], "blocked",
        "a dialog nobody has answered is a wait, by the same definition as every other"
    );
    assert_eq!(
        event["detail"], "A sandboxed command needs network access",
        "the harness's own words and no more: the payload carries no host and \
         no port, so anything more specific would be invented"
    );
    assert_eq!(
        waiting_sessions(&sandbox),
        vec!["s1".to_string()],
        "and the lamp says the operator is being waited on"
    );
    assert!(
        remind_record(&sandbox, "s1").exists(),
        "and an unanswered sandbox dialog is nudged like any other approval"
    );
}

#[test]
fn every_other_permission_prompt_on_this_arm_is_silence() {
    // THE ALLOWLIST IS THE WHOLE DISCRIMINATOR, and it is held in the binary
    // rather than trusted from the declaration's matcher: the matcher can
    // only say `permission_prompt`, which is also every ordinary tool
    // approval's type, and `PermissionRequest` has already reported those.
    // A wording change upstream turns this alert off rather than mis-firing
    // it, which is why the exact string is pinned by a test and carries the
    // Claude Code version it was measured against (2.1.272).
    for message in [
        "Claude needs your permission to use Bash",
        "a sandboxed command needs network access",
        "A sandboxed command needs network access please",
        "",
    ] {
        let sandbox = Sandbox::new(&format!("sandbox-network-silent-{}", message.len()));
        sandbox.write_config(&format!("{}{}", remind_config(300), LAMPS_ON));
        counted_channels(&sandbox);

        let output = hook_with(
            with_state_dir(&sandbox),
            &sandbox,
            "waiting",
            &permission_prompt("s1", message),
        );

        assert_eq!(output.status.code(), Some(0), "{message:?}");
        assert_eq!(
            deliveries(&sandbox, "hermes"),
            0,
            "{message:?}: a message this binary has not verified cards nobody"
        );
        assert!(
            waiting_sessions(&sandbox).is_empty(),
            "{message:?}: and arms no wait"
        );
    }
}
