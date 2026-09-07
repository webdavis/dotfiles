use super::*;

// --- what the turn said -----------------------------------------------------

#[test]
fn the_payloads_own_final_text_becomes_the_detail_without_reading_a_transcript() {
    let sandbox = Sandbox::new("hook-stop-payload-reply");
    hook(
        &sandbox,
        "stop",
        r#"{"session_id":"s1","cwd":"/a/dotfiles","transcript_path":"/nonexistent","last_assistant_message":"the payload reply"}"#,
    );
    let event = sandbox.event("hermes");
    assert_eq!(event["detail"], "the payload reply");
    assert_eq!(event["project"], "dotfiles");
}

#[test]
fn the_transcript_tail_is_the_fallback_when_the_harness_carried_no_text() {
    let sandbox = Sandbox::new("hook-stop-transcript");
    let transcript = sandbox.path("t.jsonl");
    std::fs::write(
        &transcript,
        "{\"type\":\"user\",\"message\":{\"content\":\"ask\"}}\n{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"from the transcript\"}]}}\n",
    )
    .expect("transcript");
    hook(
        &sandbox,
        "stop",
        &format!(
            r#"{{"session_id":"s1","cwd":"/a/dotfiles","transcript_path":"{}"}}"#,
            transcript.display()
        ),
    );
    assert_eq!(sandbox.event("hermes")["detail"], "from the transcript");
}

#[test]
fn a_turn_with_nothing_readable_still_notifies_with_no_detail() {
    let sandbox = Sandbox::new("hook-stop-empty");
    let output = hook(
        &sandbox,
        "stop",
        r#"{"session_id":"s1","cwd":"/a/dotfiles","last_assistant_message":"   "}"#,
    );
    assert!(output.status.success());
    let event = sandbox.event("hermes");
    assert_eq!(event["detail"], "");
    assert_eq!(event["state"], "done");
}

#[test]
fn a_condenser_line_is_used_state_and_all_and_a_blank_summary_falls_back() {
    let sandbox = Sandbox::new("hook-stop-condenser");
    let mut command = sandbox.pns();
    sandbox.stub_codex(&mut command, "asking|it wants a choice");
    hook_with(
        command,
        &sandbox,
        "stop",
        r#"{"session_id":"s1","cwd":"/a/dotfiles","last_assistant_message":"a long turn"}"#,
    );
    let event = sandbox.event("hermes");
    assert_eq!(
        event["state"], "asking",
        "the condenser may override the state"
    );
    assert_eq!(event["detail"], "it wants a choice");

    let blank = Sandbox::new("hook-stop-condenser-blank");
    let mut command = blank.pns();
    blank.stub_codex(&mut command, "done|   ");
    hook_with(
        command,
        &blank,
        "stop",
        r#"{"session_id":"s1","cwd":"/a/dotfiles","last_assistant_message":"a long turn"}"#,
    );
    assert_eq!(
        blank.event("hermes")["detail"],
        "a long turn",
        "a summary of spaces is as blank as no summary, so the reply stands"
    );
}

#[test]
fn the_re_entry_guard_keeps_a_condenser_run_from_condensing_itself() {
    let sandbox = Sandbox::new("hook-stop-reentry");
    let mut command = sandbox.pns();
    command.env("PNS_SUMMARIZING", "1");
    sandbox.stub_codex(&mut command, "asking|never asked");
    hook_with(
        command,
        &sandbox,
        "stop",
        r#"{"session_id":"s1","cwd":"/a/dotfiles","last_assistant_message":"a turn"}"#,
    );
    assert_eq!(sandbox.event("hermes")["detail"], "a turn");
}

#[test]
fn the_herdr_pane_reaches_the_event_verbatim_and_a_hostile_one_is_scrubbed_downstream() {
    let sandbox = Sandbox::new("hook-stop-pane");
    let mut command = sandbox.pns();
    command.env("HERDR_PANE_ID", "wW:p21");
    hook_with(
        command,
        &sandbox,
        "stop",
        r#"{"session_id":"s1","cwd":"/a/dotfiles","last_assistant_message":"x"}"#,
    );
    assert_eq!(sandbox.event("hermes")["pane"], "wW:p21");
}

#[test]
fn a_garbage_re_read_knob_still_notifies_and_still_exits_zero() {
    let sandbox = Sandbox::new("hook-garbage-knob");
    let mut command = sandbox.pns();
    command.env("PNS_REPLY_REREAD_ATTEMPTS", "not-a-number");
    let output = hook_with(
        command,
        &sandbox,
        "stop",
        r#"{"session_id":"s1","cwd":"/a/dotfiles","last_assistant_message":"a turn"}"#,
    );
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(sandbox.event("hermes")["detail"], "a turn");
}

#[test]
fn the_world_is_read_at_dispatch_and_not_at_the_moment_the_hook_started() {
    // THE TIMING CONTRACT, made observable. The operator taps their phone and
    // the turn then spends seconds in the condenser; by the time anything is
    // delivered the tap is the older signal and the desk is where they are.
    //
    // The marker is touched as this hook starts and the desk is stated at two
    // seconds, so the two swap places DURING the condense: a reading taken at
    // process start says mobile and cards the phone, and a reading taken at
    // dispatch says desk and raises the banner. The banner is therefore the
    // whole assertion.
    //
    // TWO, NOT ONE: ages are whole seconds and a tie goes to the desk, so a
    // desk stated at one second read Desk whenever the fresh marker's own age
    // had just rolled over to one, and a hook reading the world at start
    // passed this test about one run in twenty (measured 2026-09-01).
    let sandbox = Sandbox::new("hook-snapshot-timing");
    let marker = sandbox.path("phone.marker");
    std::fs::write(&marker, "").expect("marker");
    let bin = sandbox.path("bin");
    std::fs::create_dir_all(&bin).expect("stub bin");
    // THE MARKER IS BACKDATED RATHER THAN WAITED PAST: the condenser stub
    // re-dates it ten seconds into the past the instant it runs, so the
    // dispatch-time read is already older than the two-second desk reading
    // without this test spending any real time getting there.
    write_script(
        &bin.join("codex"),
        &format!("touch -A -000010 \"{}\"", marker.display()),
    );
    let mut command = sandbox.pns();
    command
        .env("PNS_IDLE_SECS", "2")
        .env("PNS_DESK_IDLE_SECS", "120")
        .env("PNS_PHONE_MARKER_FILE", &marker)
        .env("CODEX_BIN", bin.join("codex"))
        .env("PNS_CODEX_HOME", sandbox.path("codex-home"));
    hook_with(
        command,
        &sandbox,
        "stop",
        r#"{"session_id":"s1","cwd":"/a/dotfiles","last_assistant_message":"a turn"}"#,
    );
    assert!(
        sandbox.fired("macos-banner"),
        "the banner belongs to the desk the operator went back to"
    );
    assert!(
        !sandbox.fired("mobile"),
        "and the tap that started this turn is no longer where they are"
    );
}

#[test]
fn a_turn_whose_transcript_lands_late_is_re_read_until_it_does() {
    // The harness has not always flushed when Stop runs. The transcript is
    // EMPTY at spawn and gains its reply while the hook is between reads, so
    // collapsing the loop to a single read loses it.
    let sandbox = Sandbox::new("hook-late-flush");
    let transcript = sandbox.path("t.jsonl");
    std::fs::write(&transcript, "").expect("empty transcript");
    let mut command = sandbox.pns();
    command
        .env("PNS_REPLY_REREAD_ATTEMPTS", "8")
        .env("PNS_REPLY_REREAD_INTERVAL", "0.05");
    let mut child = spawn_hook(command, "stop");
    write_payload(
        &mut child,
        format!(
            r#"{{"session_id":"s1","cwd":"/a/dotfiles","transcript_path":"{}"}}"#,
            transcript.display()
        )
        .as_bytes(),
    );
    std::thread::sleep(std::time::Duration::from_millis(120));
    std::fs::write(
        &transcript,
        "{\"type\":\"user\",\"message\":{\"content\":\"ask\"}}\n{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"landed late\"}]}}\n",
    )
    .expect("late write");
    assert_eq!(finished_within(child, HANG_LIMIT), Some(0));
    assert_eq!(sandbox.event("hermes")["detail"], "landed late");
}

#[test]
fn a_malformed_reread_interval_falls_back_instead_of_panicking() {
    // Duration::from_secs_f64 panics on these, on a path whose contract is
    // exiting 0. The last two are FINITE and non-negative, so they passed the
    // filter that guarded the other four and panicked in the constructor
    // anyway; sol reproduced exit 101 from 1e300 on 2026-08-19.
    let sandbox = Sandbox::new("hook-bad-interval");
    let values = ["NaN", "inf", "-1", "not-a-number", "1e30", "1e300"];
    // ALL SIX SPAWNED BEFORE ANY IS WAITED ON: the property under test is
    // that each value exits 0, not that six spawns run one after another, so
    // the six process starts overlap instead of paying their own overhead
    // six times over.
    let children: Vec<_> = values
        .iter()
        .map(|value| {
            let mut command = sandbox.pns();
            command
                .env("PNS_REPLY_REREAD_INTERVAL", value)
                .env("PNS_REPLY_REREAD_ATTEMPTS", "1");
            let mut child = spawn_hook(command, "stop");
            write_payload(
                &mut child,
                br#"{"session_id":"s1","cwd":"/a/dotfiles","transcript_path":"/dev/null"}"#,
            );
            child
        })
        .collect();
    for (value, child) in values.into_iter().zip(children) {
        assert_eq!(
            finished_within(child, HANG_LIMIT),
            Some(0),
            "interval {value:?}"
        );
    }
}
