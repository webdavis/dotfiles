use super::*;

#[test]
fn a_config_change_does_not_clear_a_live_wait_on_its_own_session() {
    // LOAD-BEARING, in `an_observation_does_not_clear_a_live_wait`'s own
    // style: `blocked_marker_action("config-change")` is `End`, and the End
    // arm removes the marker UNGATED, so no `[lights]`/`[plugins.hue]` table
    // is needed for a misrouted `Attempt::First` to clear it regardless of
    // whether the lamps are configured.
    let sandbox = Sandbox::new("config-change-no-clear-own-wait");
    sandbox.write_config(&nag_config(300));
    counted_channels(&sandbox);
    std::fs::create_dir_all(sandbox.path("state/lights-blocked")).expect("lights-blocked dir");
    std::fs::write(sandbox.path("state/lights-blocked/s1"), "1700000000")
        .expect("this session's own marker");

    let output = hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "config-change",
        &config_change_payload("s1", "user_settings", None),
    );

    assert!(output.status.success());
    assert_eq!(
        deliveries(&sandbox, "hermes"),
        1,
        "the positive control, without it the marker's survival proves nothing"
    );
    assert_eq!(
        waiting_sessions(&sandbox),
        vec!["s1".to_string()],
        "a config-change observation must not clear its own session's live wait"
    );

    // THE CONTROL, run AFTER on the SAME sandbox: proves a First `stop` event
    // for this session DOES clear the marker, so the assertion above is not
    // vacuously true under every attempt.
    hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "stop",
        r#"{"session_id":"s1","cwd":"/a/dotfiles"}"#,
    );
    assert!(
        waiting_sessions(&sandbox).is_empty(),
        "the control: a First `stop` event for this session clears its own wait"
    );
}

#[test]
fn a_config_change_writes_no_activity_line() {
    let sandbox = Sandbox::new("config-change-no-activity");
    sandbox.write_config(&nag_config(300));
    counted_channels(&sandbox);
    let activity_before = state_lines(&sandbox, "activity");

    let output = hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "config-change",
        &config_change_payload("s1", "user_settings", None),
    );

    assert!(output.status.success());
    assert_eq!(
        deliveries(&sandbox, "hermes"),
        1,
        "the positive control fired"
    );
    assert_eq!(
        state_lines(&sandbox, "activity"),
        activity_before,
        "an observation writes no activity-ring line"
    );

    hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "stop",
        r#"{"session_id":"s-control"}"#,
    );
    assert_ne!(
        state_lines(&sandbox, "activity"),
        activity_before,
        "the control: a First `stop` event writes an activity-ring line"
    );
}

#[test]
fn a_config_change_renews_no_loop_lease() {
    let sandbox = Sandbox::new("config-change-no-lease");
    sandbox.write_config(&nag_config(300));
    counted_channels(&sandbox);
    let lease_dir = sandbox.path("state/lights-loop");
    std::fs::create_dir_all(&lease_dir).expect("lease dir");
    std::fs::write(lease_dir.join("wW:p1"), "100\n").expect("an old lease");

    let mut command = with_state_dir(&sandbox);
    command.env("HERDR_PANE_ID", "wW:p1");
    let output = hook_with(
        command,
        &sandbox,
        "config-change",
        &config_change_payload("s1", "user_settings", None),
    );

    assert!(output.status.success());
    assert_eq!(
        deliveries(&sandbox, "hermes"),
        1,
        "the positive control fired"
    );
    assert_eq!(
        std::fs::read_to_string(lease_dir.join("wW:p1")).unwrap_or_default(),
        "100\n",
        "an observation renews no loop lease"
    );

    let mut control = with_state_dir(&sandbox);
    control.env("HERDR_PANE_ID", "wW:p1");
    hook_with(control, &sandbox, "stop", r#"{"session_id":"s-control"}"#);
    assert_ne!(
        std::fs::read_to_string(lease_dir.join("wW:p1")).unwrap_or_default(),
        "100\n",
        "the control: a First `stop` event on this pane renews the loop lease"
    );
}

#[test]
fn a_config_change_moves_no_presence_edge() {
    // THE OBSERVATION IS CHECKED AGAINST THE STALE SEED DIRECTLY, never
    // against a marker a same-second control call just wrote: see
    // `an_observation_moves_no_presence_edge`'s own comment for why running
    // the control before the observation would let a misrouted `Attempt::First`
    // pass for the wrong reason under `mark_present`'s own `held >= now` guard.
    let sandbox = Sandbox::new("config-change-no-presence-edge");
    sandbox.write_config(&nag_config(300));
    counted_channels(&sandbox);
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    std::fs::write(sandbox.path("state/last-present"), "1").expect("seed");

    let mut command = with_state_dir(&sandbox);
    command.env("PNS_IDLE_SECS", "0");
    let output = hook_with(
        command,
        &sandbox,
        "config-change",
        &config_change_payload("s1", "user_settings", None),
    );

    assert!(output.status.success());
    assert_eq!(
        deliveries(&sandbox, "hermes"),
        1,
        "the positive control fired"
    );
    assert_eq!(
        std::fs::read_to_string(sandbox.path("state/last-present")).unwrap_or_default(),
        "1",
        "an observation never claims the return moment"
    );

    let mut control = with_state_dir(&sandbox);
    control.env("PNS_IDLE_SECS", "0");
    hook_with(control, &sandbox, "stop", r#"{"session_id":"s-control"}"#);
    assert_ne!(
        std::fs::read_to_string(sandbox.path("state/last-present")).unwrap_or_default(),
        "1",
        "the control: a First `stop` event under this env advances the presence edge"
    );
}

#[test]
fn a_config_change_registers_no_lights_tick() {
    let sandbox = Sandbox::new("config-change-no-lights-tick");
    sandbox.write_config(&format!("{LAMPS_ON}[plugins.hermes]\nenabled = true\n"));
    counted_channels(&sandbox);

    let output = hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "config-change",
        &config_change_payload("s1", "user_settings", None),
    );

    assert!(output.status.success());
    assert_eq!(
        deliveries(&sandbox, "hermes"),
        1,
        "the positive control fired"
    );
    assert!(
        spool_entries(&sandbox).is_empty(),
        "an observation registers no lights tick"
    );

    hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "stop",
        r#"{"session_id":"s-control"}"#,
    );
    assert!(
        !spool_entries(&sandbox).is_empty(),
        "the control: a First `stop` event under this lamps-live config registers the lights tick"
    );
}

#[test]
fn a_config_change_observation_journals_no_missed_notification() {
    let sandbox = Sandbox::new("config-change-journals-no-miss");
    sandbox.write_config(&nag_config(300));
    counted_channels(&sandbox);
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    let expiry = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("a clock past 1970")
        .as_secs()
        + 600;
    std::fs::write(sandbox.path("state/quiet-until"), format!("{expiry}\n")).expect("the mute");
    let journal = sandbox.path("state/missed-notifications");

    let output = hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "config-change",
        &config_change_payload("s1", "user_settings", None),
    );

    assert!(output.status.success());
    assert_eq!(
        deliveries(&sandbox, "hermes"),
        1,
        "the positive control fired: hermes is the durable log and rides even a muted event"
    );
    assert!(!journal.exists(), "an observation writes no journal entry");

    hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "stop",
        r#"{"session_id":"s-control"}"#,
    );
    assert!(
        journal.exists(),
        "the control: a First `stop` event under this config journals a miss"
    );
}

#[test]
fn a_config_change_observation_replays_no_journal_entry() {
    let sandbox = Sandbox::new("config-change-replays-no-entry");
    sandbox.write_config(&nag_config(300));
    counted_channels(&sandbox);
    let journal = sandbox.path("state/missed-notifications");
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    let seeded = "{\"at\":1756499000,\"agent\":\"claude\",\"state\":\"done\",\
                  \"project\":\"p\",\"branch\":\"b\",\"detail\":\"planted\"}\n";
    std::fs::write(&journal, seeded).expect("the journal");

    let mut command = with_state_dir(&sandbox);
    command.env("PNS_IDLE_SECS", "0");
    let output = hook_with(
        command,
        &sandbox,
        "config-change",
        &config_change_payload("s1", "user_settings", None),
    );

    assert!(output.status.success());
    assert_eq!(
        std::fs::read_to_string(&journal).unwrap_or_default(),
        seeded,
        "an observation replays no journal entry"
    );
    assert_eq!(
        deliveries(&sandbox, "hermes"),
        1,
        "the positive control fired"
    );

    let mut control = with_state_dir(&sandbox);
    control.env("PNS_IDLE_SECS", "0");
    hook_with(control, &sandbox, "stop", r#"{"session_id":"s-control"}"#);
    assert!(
        !journal.exists(),
        "the control: a First `stop` event under this env consumes the journal"
    );
}
