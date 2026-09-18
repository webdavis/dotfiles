use super::*;

const FUTURE: &str = "18446744073709551615\n";
const REPLY: &str = r#"{"session_id":"s1","last_assistant_message":"Which option?"}"#;

fn setup(name: &str, lease: Option<(&str, &str)>) -> Sandbox {
    let sandbox = Sandbox::new(name);
    sandbox.write_config(&format!("{}{LAMPS_ON}", support::STUB_CHANNELS));
    if let Some((pane, epoch)) = lease {
        let directory = sandbox.path("state/lights-loop");
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join(pane), epoch).unwrap();
    }
    sandbox
}

fn asking(sandbox: &Sandbox) {
    let mut command = with_state_dir(sandbox);
    command.env("HERDR_PANE_ID", "wW:p1");
    sandbox.stub_herdr(&mut command, false);
    sandbox.stub_codex(&mut command, "asking|Which option?");
    let output = hook_with(command, sandbox, "stop", REPLY);
    assert_eq!(output.status.code(), Some(0), "{:?}", output.stderr);
    assert_eq!(sandbox.event("hermes")["state"], "asking");
}

#[test]
fn a_live_pane_loop_keeps_a_condensed_question_from_arming_blocked() {
    let sandbox = setup("loop-asking-live", Some(("wW:p1", FUTURE)));
    asking(&sandbox);
    assert!(waiting_sessions(&sandbox).is_empty());
}

#[test]
fn a_condensed_question_without_a_loop_arms_blocked() {
    let sandbox = setup("loop-asking-absent", None);
    asking(&sandbox);
    assert_eq!(waiting_sessions(&sandbox), ["s1"]);
}

#[test]
fn an_expired_loop_does_not_hide_a_question_even_though_the_event_renews_it() {
    let sandbox = setup("loop-asking-expired", Some(("wW:p1", "0\n")));
    asking(&sandbox);
    assert_eq!(waiting_sessions(&sandbox), ["s1"]);
    assert_ne!(
        std::fs::read_to_string(sandbox.path("state/lights-loop/wW:p1")).unwrap(),
        "0\n"
    );
}

#[test]
fn an_invalid_loop_epoch_does_not_hide_a_condensed_question() {
    let sandbox = setup("loop-asking-invalid", Some(("wW:p1", "invalid\n")));
    asking(&sandbox);
    assert_eq!(waiting_sessions(&sandbox), ["s1"]);
}

#[test]
fn another_panes_live_loop_does_not_hide_a_condensed_question() {
    let sandbox = setup("loop-asking-other-pane", Some(("wW:p2", FUTURE)));
    asking(&sandbox);
    assert_eq!(waiting_sessions(&sandbox), ["s1"]);
}

#[test]
fn a_live_loop_does_not_suppress_real_hook_waits() {
    for event in ["asked", "blocked"] {
        let sandbox = setup(&format!("loop-real-{event}"), Some(("wW:p1", FUTURE)));
        let mut command = with_state_dir(&sandbox);
        command.env("HERDR_PANE_ID", "wW:p1");
        sandbox.stub_herdr(&mut command, false);
        let output = hook_with(
            command,
            &sandbox,
            event,
            r#"{"session_id":"s1","message":"May I?"}"#,
        );
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(waiting_sessions(&sandbox), ["s1"], "{event}");
        assert_eq!(sandbox.event("hermes")["state"], event);
    }
}

#[test]
fn a_condensed_question_does_not_clear_or_renew_an_existing_real_wait_during_a_loop() {
    let sandbox = setup("loop-asking-existing-wait", Some(("wW:p1", FUTURE)));
    let directory = sandbox.path("state/lights-blocked");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("s1"), "100\n").unwrap();
    asking(&sandbox);
    assert_eq!(
        std::fs::read_to_string(directory.join("s1")).unwrap(),
        "100\n"
    );
}

#[test]
fn finished_turns_keep_their_news_and_clear_blocked_during_a_loop() {
    for (event, state) in [("stop", "done"), ("stop-failure", "failed")] {
        let sandbox = setup(&format!("loop-finished-{state}"), Some(("wW:p1", FUTURE)));
        let directory = sandbox.path("state/lights-blocked");
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("s1"), "100\n").unwrap();
        let mut command = with_state_dir(&sandbox);
        command.env("HERDR_PANE_ID", "wW:p1");
        sandbox.stub_herdr(&mut command, false);
        sandbox.stub_codex(&mut command, "done|Finished");
        let output = hook_with(command, &sandbox, event, REPLY);
        assert_eq!(output.status.code(), Some(0));
        assert!(waiting_sessions(&sandbox).is_empty(), "{state}");
        assert_eq!(sandbox.event("hermes")["state"], state);
    }
}
