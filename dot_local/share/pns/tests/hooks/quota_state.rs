use super::*;

#[test]
fn no_quota_type_clears_a_live_wait_on_its_own_session() {
    // THE SAME SESSION, not another one. `update_blocked_marker` only ever
    // touches the PAYLOAD'S OWN session file (main.rs), so seeding a
    // different session's marker proves nothing: it would survive a misrouted
    // `Attempt::First` exactly as it survives a correct `Observation`, since
    // neither ever reaches a file named for a session that never sent an
    // event. `stale` is excluded from this loop: it arms its own marker
    // directly through `arm_quota_stale_wait`, a separate mechanism this test
    // does not exercise (see
    // `quota_auto_resume_stale_arms_the_needs_marker_for_its_own_session`).
    for notification_type in ["quota_auto_resume_fired", "quota_auto_resume_disabled"] {
        let sandbox = Sandbox::new(&format!("quota-no-clear-own-{notification_type}"));
        sandbox.write_config(&nag_config(300));
        counted_channels(&sandbox);
        std::fs::create_dir_all(sandbox.path("state/lights-blocked")).expect("lights-blocked dir");
        std::fs::write(sandbox.path("state/lights-blocked/s1"), "1700000000")
            .expect("this session's own marker");

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
            "{notification_type}: the positive control, without it the marker's survival proves nothing"
        );
        assert_eq!(
            waiting_sessions(&sandbox),
            vec!["s1".to_string()],
            "{notification_type} must not clear its own session's live wait"
        );
    }
}

#[test]
fn no_quota_type_arms_unread_news() {
    for notification_type in QUOTA_TYPES {
        let sandbox = Sandbox::new(&format!("quota-no-news-{notification_type}"));
        sandbox.write_config(&nag_config(300));
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
            !sandbox.path("state/lights-news").exists(),
            "{notification_type}: arms no unread-news lamp"
        );
    }
}

#[test]
fn no_quota_type_writes_an_activity_line() {
    for notification_type in QUOTA_TYPES {
        let sandbox = Sandbox::new(&format!("quota-no-activity-{notification_type}"));
        sandbox.write_config(&nag_config(300));
        counted_channels(&sandbox);
        let activity_before = state_lines(&sandbox, "activity");

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
        assert_eq!(
            state_lines(&sandbox, "activity"),
            activity_before,
            "{notification_type}: writes no activity-ring line"
        );
    }
}

#[test]
fn a_quota_observation_journals_no_missed_notification() {
    // A delivered card is not a miss, so `nag_config`'s bare three channels
    // never reach `record_missed`'s `was_missed` branch whichever attempt
    // fires: the assertion below would hold for a card that landed just as
    // much as for one an Observation correctly withheld from the journal.
    // The operator's own mute is what zeroes both the banner and the phone
    // card unconditionally (see `an_observation_journals_no_missed_notification`,
    // model-switch's own version of this same control), which is what a
    // First-attempt run under the SAME mute proves is reachable here.
    let sandbox = Sandbox::new("quota-journals-no-miss");
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
        "quota",
        &quota_payload("s1", "quota_auto_resume_fired", "m"),
    );

    assert!(output.status.success());
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
fn a_quota_observation_replays_no_journal_entry() {
    // A seeded, already-replayable entry is what `claim_journal` would
    // otherwise consume: without one, "the journal survives" is true whether
    // or not the guard works, because there is nothing in it to lose.
    let sandbox = Sandbox::new("quota-replays-no-entry");
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
        "quota",
        &quota_payload("s1", "quota_auto_resume_fired", "m"),
    );

    assert!(output.status.success());
    // CHECKED BEFORE THE DELIVERY COUNT, deliberately: a misrouted
    // `Attempt::First` here also runs `replay_missed`, which delivers a
    // SECOND card off the seeded entry, so a delivery-count assertion ahead
    // of this one would catch the mutation for the wrong reason and never
    // reach the journal check at all.
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
fn a_quota_observation_registers_no_lights_tick() {
    // A LAMPS-LIVE case: `register_lights_tick` is gated on `lamps_live`
    // (main.rs), so `nag_config`'s bare three channels never reach it
    // whichever attempt fires. This needs its own `[lights]`/`[plugins.hue]`
    // table, `LAMPS_ON`'s own fixture, the way model-switch's
    // `an_observation_registers_no_lights_tick` needs it.
    let sandbox = Sandbox::new("quota-no-lights-tick");
    sandbox.write_config(&format!("{LAMPS_ON}[plugins.hermes]\nenabled = true\n"));
    counted_channels(&sandbox);

    let output = hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "quota",
        &quota_payload("s1", "quota_auto_resume_fired", "m"),
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

#[test]
fn no_quota_type_renews_a_loop_lease() {
    for notification_type in QUOTA_TYPES {
        let sandbox = Sandbox::new(&format!("quota-no-lease-{notification_type}"));
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
            "quota",
            &quota_payload("s1", notification_type, "m"),
        );

        assert!(output.status.success(), "{notification_type}");
        assert_eq!(
            deliveries(&sandbox, "hermes"),
            1,
            "{notification_type}: the positive control fired"
        );
        assert_eq!(
            std::fs::read_to_string(lease_dir.join("wW:p1")).unwrap_or_default(),
            "100\n",
            "{notification_type}: renews no loop lease"
        );
    }
}

#[test]
fn no_quota_type_moves_the_presence_edge() {
    for notification_type in QUOTA_TYPES {
        let sandbox = Sandbox::new(&format!("quota-no-presence-{notification_type}"));
        sandbox.write_config(&nag_config(300));
        counted_channels(&sandbox);
        std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
        std::fs::write(sandbox.path("state/last-present"), "1").expect("seed");

        // THE OBSERVATION IS CHECKED AGAINST THE STALE SEED FIRST, never
        // against a marker a same-second control call already wrote: running
        // the control before the observation let a misrouted `Attempt::First`
        // land in the same wall-clock second as the control, where
        // `mark_present`'s own `held >= now` guard made the second write
        // inert too, passing the "unchanged from the control" comparison for
        // the wrong reason (measured: this exact ordering let the
        // misrouting mutation stay green). Checking against the literal
        // seed, then running the control AFTER, avoids the race regardless
        // of timing.
        let mut command = with_state_dir(&sandbox);
        command.env("PNS_IDLE_SECS", "0");
        let output = hook_with(
            command,
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
        assert_eq!(
            std::fs::read_to_string(sandbox.path("state/last-present")).unwrap_or_default(),
            "1",
            "{notification_type}: an observation never claims the return moment"
        );

        // THE CONTROL, run AFTER on the SAME stale seed: proves a First
        // `done` event under this exact env DOES advance the presence edge,
        // so the assertion above is not vacuously true under every attempt.
        let mut control = with_state_dir(&sandbox);
        control.env("PNS_IDLE_SECS", "0");
        hook_with(control, &sandbox, "stop", r#"{"session_id":"s-control"}"#);
        assert_ne!(
            std::fs::read_to_string(sandbox.path("state/last-present")).unwrap_or_default(),
            "1",
            "{notification_type}: the control: a First `done` event advances the presence edge"
        );
    }
}
