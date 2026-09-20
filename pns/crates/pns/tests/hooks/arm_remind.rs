use super::*;

// --- `arm-remind`, the schedule-only word -----------------------------------
//
// omp's own generated extension reports `tool_approval_requested` straight to
// the moshi daemon socket, so a `blocked` hook for that harness would raise a
// SECOND card for the same approval. `arm-remind` is the same arm as
// `blocked` minus the notification and minus the moshi forward: it schedules
// the nudge and nothing else.

#[test]
fn arm_remind_schedules_a_nudge_and_raises_no_card_of_its_own() {
    let sandbox = Sandbox::new("arm-remind-schedules-silently");
    sandbox.write_config(&remind_config(300));
    counted_channels(&sandbox);
    let mut command = sandbox.pns_stateful();
    sandbox.stub_moshi(&mut command, 0);

    let output = hook_flagged(
        command,
        "arm-remind",
        &["--remind"],
        "{\"message\":\"Bash: cargo test\",\"session_id\":\"s1\"}\n",
    );

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        deliveries(&sandbox, "hermes"),
        0,
        "the caller's own extension already carded this approval; arming it again \
         must not add a second"
    );
    assert!(
        submissions(&sandbox).is_empty(),
        "and no round trip to moshi either: this word never forwards"
    );
    let record = remind_record(&sandbox, "s1");
    let raw = std::fs::read_to_string(&record).expect("a record");
    let parsed: serde_json::Value = serde_json::from_str(&raw).expect("the record is JSON");
    assert_eq!(parsed["detail"], "Bash: cargo test");
}

#[test]
fn resolved_clears_a_reminder_arm_remind_armed() {
    let sandbox = Sandbox::new("arm-remind-cleared-by-resolved");
    sandbox.write_config(&remind_config(300));
    counted_channels(&sandbox);
    let mut command = sandbox.pns_stateful();
    sandbox.stub_moshi(&mut command, 0);
    hook_flagged(
        command,
        "arm-remind",
        &["--remind"],
        "{\"message\":\"Bash: cargo test\",\"session_id\":\"s1\"}\n",
    );
    assert!(remind_record(&sandbox, "s1").exists());

    hook_with(
        sandbox.pns_stateful(),
        &sandbox,
        "resolved",
        "{\"session_id\":\"s1\"}\n",
    );

    assert!(
        !remind_record(&sandbox, "s1").exists(),
        "the answered signal clears an arm-only reminder the same as any other"
    );
}
