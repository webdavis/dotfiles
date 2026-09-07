use super::*;

#[test]
fn an_observation_does_not_clear_a_live_wait() {
    // LOAD-BEARING. `blocked_marker_action("model-switch")` is `End`
    // (lights.rs:690-696), and the End arm removes the marker UNGATED
    // (main.rs:571-573), so this needs no `[lights]`/`[plugins.hue]` table at
    // all: if the guard ever misrouted this as First, the marker would be
    // gone regardless of whether the lamps are configured.
    let sandbox = Sandbox::new("observation-live-wait");
    sandbox.write_config(&nag_config(300));
    counted_channels(&sandbox);
    std::fs::create_dir_all(sandbox.path("state/lights-blocked")).expect("lights-blocked dir");
    std::fs::write(sandbox.path("state/lights-blocked/s1"), "1700000000").expect("the marker");
    let missed_before = state_lines(&sandbox, "missed-notifications");
    let spool_before = spool_entries(&sandbox);

    let output = hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "model-switch",
        &model_switch_payload("s1", "auto"),
    );

    assert!(output.status.success(), "an observation still exits 0");
    assert_eq!(
        deliveries(&sandbox, "hermes"),
        1,
        "the positive control: without a delivery the marker's survival proves nothing"
    );
    assert_eq!(
        waiting_sessions(&sandbox),
        vec!["s1".to_string()],
        "an observation must not clear a live wait"
    );
    assert_eq!(
        state_lines(&sandbox, "missed-notifications"),
        missed_before,
        "an observation writes no journal entry"
    );
    assert_eq!(
        spool_entries(&sandbox),
        spool_before,
        "an observation registers no lights tick"
    );
}

#[test]
fn an_observation_arms_no_unread_news() {
    // `record_news` is deliberately UNGATED on the lamp switches, so this
    // needs no lamp config either: an observation must not write it whether
    // or not the machine has lamps at all.
    let sandbox = Sandbox::new("observation-no-unread-news");
    sandbox.write_config(&nag_config(300));
    counted_channels(&sandbox);
    let missed_before = state_lines(&sandbox, "missed-notifications");
    let spool_before = spool_entries(&sandbox);

    let output = hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "model-switch",
        &model_switch_payload("s1", "auto"),
    );

    assert!(output.status.success(), "an observation still exits 0");
    assert_eq!(
        deliveries(&sandbox, "hermes"),
        1,
        "the positive control fired"
    );
    assert!(
        !sandbox.path("state/lights-news").exists(),
        "an observation arms no unread-news lamp"
    );
    assert_eq!(state_lines(&sandbox, "missed-notifications"), missed_before);
    assert_eq!(spool_entries(&sandbox), spool_before);
}

#[test]
fn an_observation_writes_no_activity_line() {
    let sandbox = Sandbox::new("observation-no-activity-line");
    sandbox.write_config(&nag_config(300));
    counted_channels(&sandbox);
    let activity_before = state_lines(&sandbox, "activity");
    let missed_before = state_lines(&sandbox, "missed-notifications");
    let spool_before = spool_entries(&sandbox);

    let output = hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "model-switch",
        &model_switch_payload("s1", "auto"),
    );

    assert!(output.status.success(), "an observation still exits 0");
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
    assert_eq!(state_lines(&sandbox, "missed-notifications"), missed_before);
    assert_eq!(spool_entries(&sandbox), spool_before);
}

#[test]
fn an_observation_moves_no_presence_edge() {
    // S3: `Sandbox::pns` sets PNS_IDLE_SECS=99999 (Away), and `mark_present`
    // returns before writing while away, so a First-routed observation would
    // ALSO leave `last-present` alone under the suite's default env. Force
    // Present with PNS_IDLE_SECS=0.
    //
    // THE OBSERVATION IS CHECKED AGAINST THE STALE SEED DIRECTLY, never
    // against a marker a same-second control call just wrote: two hook
    // spawns close enough together can land in the same wall-clock second,
    // and `mark_present`'s own `held >= now` guard would then leave a SECOND
    // First event's write inert too, making a "does the observation move it
    // further than the control did" comparison pass for the wrong reason
    // (measured: it let a mutant that misroutes this arm as First stay
    // green). Seeding a stale epoch and asserting it is UNCHANGED avoids the
    // race regardless of timing.
    let sandbox = Sandbox::new("observation-no-presence-edge");
    sandbox.write_config(&nag_config(300));
    counted_channels(&sandbox);
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    std::fs::write(sandbox.path("state/last-present"), "1").expect("seed");
    let missed_before = state_lines(&sandbox, "missed-notifications");
    let spool_before = spool_entries(&sandbox);

    let mut command = with_state_dir(&sandbox);
    command.env("PNS_IDLE_SECS", "0");
    let output = hook_with(
        command,
        &sandbox,
        "model-switch",
        &model_switch_payload("s1", "auto"),
    );

    assert!(output.status.success(), "an observation still exits 0");
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
    assert_eq!(state_lines(&sandbox, "missed-notifications"), missed_before);
    assert_eq!(spool_entries(&sandbox), spool_before);

    // THE CONTROL, run AFTER on the SAME sandbox and the SAME stale seed:
    // proves a First `done` event under this exact env DOES advance the
    // marker, so the assertion above is not vacuously true under every
    // attempt.
    let mut control = with_state_dir(&sandbox);
    control.env("PNS_IDLE_SECS", "0");
    hook_with(control, &sandbox, "stop", r#"{"session_id":"s-control"}"#);
    assert_ne!(
        std::fs::read_to_string(sandbox.path("state/last-present")).unwrap_or_default(),
        "1",
        "the control: a First `done` event advances the presence edge"
    );
}

#[test]
fn an_observation_renews_no_loop_lease() {
    let sandbox = Sandbox::new("observation-no-lease-renewal");
    sandbox.write_config(&nag_config(300));
    counted_channels(&sandbox);
    let lease_dir = sandbox.path("state/lights-loop");
    std::fs::create_dir_all(&lease_dir).expect("lease dir");
    std::fs::write(lease_dir.join("wW:p1"), "100\n").expect("an old lease");
    let missed_before = state_lines(&sandbox, "missed-notifications");
    let spool_before = spool_entries(&sandbox);

    let mut command = with_state_dir(&sandbox);
    command.env("HERDR_PANE_ID", "wW:p1");
    let output = hook_with(
        command,
        &sandbox,
        "model-switch",
        &model_switch_payload("s1", "auto"),
    );

    assert!(output.status.success(), "an observation still exits 0");
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
    assert_eq!(state_lines(&sandbox, "missed-notifications"), missed_before);
    assert_eq!(spool_entries(&sandbox), spool_before);
}

#[test]
fn an_observation_journals_no_missed_notification() {
    // SOL 2a: the five negative assertions above prove nothing about
    // `record_missed` by themselves. `was_missed` needs BOTH the plan's
    // banner and phone card false, and those two are the SURFACE MATRIX's
    // own output: Away always plans a card and Desk with an unreadable pane
    // always plans a banner, whether or not a channel exists to carry it, so
    // no combination of enabled plugins alone reaches this. The operator's
    // own mute is the one thing that zeroes both unconditionally, which is
    // what a First-attempt control proves is reachable under it.
    let sandbox = Sandbox::new("observation-no-journal-write");
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
        "model-switch",
        &model_switch_payload("s1", "auto"),
    );

    assert!(output.status.success(), "an observation still exits 0");
    assert_eq!(
        deliveries(&sandbox, "hermes"),
        1,
        "the positive control fired: hermes is the durable log and rides even a muted event"
    );
    assert!(!journal.exists(), "an observation writes no journal entry");

    // THE CONTROL, run AFTER on the SAME sandbox: proves a First `stop`
    // event under this exact muted config DOES journal a miss, so the
    // assertion above is not vacuously true under every attempt.
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
fn an_observation_replays_no_journal_entry() {
    // SOL 2b: `should_replay` needs the plan to decorate (macos-banner or
    // mobile), which `nag_config`'s enabled plugins do at the desk, and a
    // seeded entry is what `claim_journal` would otherwise consume: without
    // one, "the journal survives" is true whether or not the guard works,
    // because there is nothing in it to lose.
    let sandbox = Sandbox::new("observation-no-journal-replay");
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
        "model-switch",
        &model_switch_payload("s1", "auto"),
    );

    assert!(output.status.success(), "an observation still exits 0");
    assert_eq!(
        deliveries(&sandbox, "hermes"),
        1,
        "the positive control fired"
    );
    assert_eq!(
        std::fs::read_to_string(&journal).unwrap_or_default(),
        seeded,
        "an observation replays no journal entry"
    );

    // THE CONTROL, run AFTER on the SAME seeded journal: proves a First
    // `stop` event under this exact env DOES consume it, so the assertion
    // above is not vacuously true under every attempt.
    let mut control = with_state_dir(&sandbox);
    control.env("PNS_IDLE_SECS", "0");
    hook_with(control, &sandbox, "stop", r#"{"session_id":"s-control"}"#);
    assert!(
        !journal.exists(),
        "the control: a First `stop` event under this env consumes the journal"
    );
}

#[test]
fn an_observation_registers_no_lights_tick() {
    // SOL 2c: `nag_config`'s three channels enable no lamps at all, so tick
    // registration cannot run under it whichever attempt fires. This needs
    // its own `[lights]`/`[plugins.hue]` table, LAMPS_ON's own fixture.
    let sandbox = Sandbox::new("observation-no-lights-tick");
    sandbox.write_config(&format!("{LAMPS_ON}[plugins.hermes]\nenabled = true\n"));
    counted_channels(&sandbox);

    let output = hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "model-switch",
        &model_switch_payload("s1", "auto"),
    );

    assert!(output.status.success(), "an observation still exits 0");
    assert_eq!(
        deliveries(&sandbox, "hermes"),
        1,
        "the positive control fired"
    );
    assert!(
        spool_entries(&sandbox).is_empty(),
        "an observation registers no lights tick"
    );

    // THE CONTROL, run AFTER on the SAME sandbox: proves a First `stop`
    // event under this exact lamps-live config DOES register the tick.
    hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "stop",
        r#"{"session_id":"s-control"}"#,
    );
    assert!(
        !spool_entries(&sandbox).is_empty(),
        "the control: a First `stop` event under this config registers the lights tick"
    );
}
