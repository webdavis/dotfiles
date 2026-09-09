use super::*;

// --- W6: the bounded, state-only policy-settings audit trail ----------------

#[test]
fn a_policy_settings_change_is_recorded_to_a_bounded_audit_trail() {
    let sandbox = Sandbox::new("config-change-policy-audit-write");
    sandbox.write_config(&nag_config(300));
    counted_channels(&sandbox);

    let output = hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "config-change",
        &config_change_payload("s1", "policy_settings", Some("/etc/claude/policy.json")),
    );

    assert!(output.status.success());
    assert_eq!(
        deliveries(&sandbox, "hermes"),
        1,
        "the ordinary observation card still fires, on top of the audit line"
    );
    let recorded = stored_records::text(&sandbox, "policy_audit");
    let lines: Vec<&str> = recorded.lines().collect();
    assert_eq!(
        lines.len(),
        1,
        "one received policy change, one line: {recorded:?}"
    );
    assert!(
        lines[0].contains("/etc/claude/policy.json"),
        "the record names the changed file: {recorded:?}"
    );
}

#[test]
fn a_non_policy_config_change_writes_no_policy_audit_entry() {
    // ONLY `policy_settings` OUTLIVES THE DECISION RING. The other four
    // sources are still logged as ordinary observations, but they must not
    // start a second durable file this binary has no bound in mind for.
    let sandbox = Sandbox::new("config-change-no-policy-audit-for-others");
    sandbox.write_config(&nag_config(300));
    counted_channels(&sandbox);

    for source in [
        "user_settings",
        "project_settings",
        "local_settings",
        "skills",
    ] {
        let output = hook_with(
            with_state_dir(&sandbox),
            &sandbox,
            "config-change",
            &config_change_payload("s1", source, Some("/a/file.json")),
        );
        assert!(output.status.success(), "{source}");
    }

    assert_eq!(
        deliveries(&sandbox, "hermes"),
        4,
        "the four ordinary cards fired"
    );
    assert!(
        stored_records::text(&sandbox, "policy_audit").is_empty(),
        "only a policy_settings change writes the audit trail"
    );

    // THE SAME-SANDBOX CONTROL, in `a_non_auto_model_switch_source_...`'s own
    // style. The absence above is meaningless on its own: deleting the whole
    // audit writer would leave it holding too, since nothing else in this
    // test ever asks the writer to run. Firing ONE `policy_settings` event
    // now, on the same sandbox, proves the writer was reachable the whole
    // time and the four sources above really were what kept it silent.
    let control = hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "config-change",
        &config_change_payload("s1", "policy_settings", Some("/etc/claude/policy.json")),
    );
    assert!(control.status.success());
    assert!(
        !stored_records::text(&sandbox, "policy_audit").is_empty(),
        "the control: a policy_settings event under this same setup writes the trail"
    );
}

#[test]
fn the_policy_settings_audit_trail_is_bounded_and_drops_the_oldest_entry() {
    // THE TRAIL'S OWN DEPTH, stated here rather than imported: a test that
    // read the constant it is checking would agree with any value the source
    // held. Twenty is `main.rs`'s `POLICY_SETTINGS_AUDIT_KEPT`.
    const POLICY_SETTINGS_AUDIT_KEPT: usize = 20;
    let sandbox = Sandbox::new("config-change-policy-audit-bound");
    sandbox.write_config(&nag_config(300));
    counted_channels(&sandbox);
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    let planted: String = (0..POLICY_SETTINGS_AUDIT_KEPT)
        .map(|which| format!("1756499000 session=s0 file=planted-{which}\n"))
        .collect();
    std::fs::write(sandbox.path("state/policy-settings-audit"), planted).expect("the audit trail");

    let output = hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "config-change",
        &config_change_payload("s1", "policy_settings", Some("/etc/claude/policy.json")),
    );

    assert!(output.status.success());
    let recorded = stored_records::text(&sandbox, "policy_audit");
    let lines: Vec<&str> = recorded.lines().collect();
    assert_eq!(
        lines.len(),
        POLICY_SETTINGS_AUDIT_KEPT,
        "the trail keeps its own bound rather than growing without limit: {recorded:?}"
    );
    // PINNED AS THE WINDOW, not as the absence of one string: the planted
    // lines end at their own number, so `planted-0` is a prefix of nothing
    // and any absence check for it holds whichever end the prune kept.
    assert!(
        lines[0].ends_with("file=planted-1"),
        "the window kept is the NEWEST twenty, so the oldest entry is the dropped one: {recorded:?}"
    );
    assert!(
        lines
            .last()
            .expect("a line")
            .contains("/etc/claude/policy.json"),
        "the newest entry is this event's: {recorded:?}"
    );
}

#[test]
fn two_policy_settings_changes_racing_the_prune_lose_neither_line() {
    // The old file-ring bug read a window, let a sibling publish, then
    // published its stale window and lost the sibling's line. Its delay
    // variable exercised that file-lock interleaving; SQLite has no such
    // delay. Import the seed first, then start two owned hook processes
    // before either receives input, keeping both new lines and exactly the
    // newest twenty entries.
    //
    // THE RACE IS THE SPAWN ORDER, NOT A CLOCK. Both children exist and are fed
    // before either is waited on, which is what makes them contend. The waits
    // below are therefore ceilings on a subject that never signals, and each
    // child gets its OWN generous one: a single shared budget spent by the first
    // child left the second almost none of it, and a loaded CI runner failed the
    // test on scheduling rather than on the behavior it pins.
    const POLICY_SETTINGS_AUDIT_KEPT: usize = 20;
    let sandbox = Sandbox::new("config-change-policy-audit-two-racers");
    sandbox.write_config(&nag_config(300));
    counted_channels(&sandbox);
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    let planted: String = (0..POLICY_SETTINGS_AUDIT_KEPT)
        .map(|which| format!("1756499000 session=s0 file=planted-{which}\n"))
        .collect();
    std::fs::write(sandbox.path("state/policy-settings-audit"), planted).expect("the audit trail");

    assert!(
        pns_adapters::SqliteStore::for_records(sandbox.state())
            .import_legacy()
            .expect("the initial audit import")
            .is_empty()
    );
    let mut slow_command = with_state_dir(&sandbox);
    slow_command.args(["hook", "config-change"]);
    let mut slow =
        captured_child::CapturedChild::spawn(&mut slow_command).expect("the first owned hook");
    let mut fast_command = with_state_dir(&sandbox);
    fast_command.args(["hook", "config-change"]);
    let mut fast =
        captured_child::CapturedChild::spawn(&mut fast_command).expect("the second owned hook");
    assert!(slow.child.try_wait().expect("first child state").is_none());
    assert!(fast.child.try_wait().expect("second child state").is_none());
    for (child, file) in [(&mut slow, "racer-slow"), (&mut fast, "racer-fast")] {
        child
            .child
            .stdin
            .take()
            .expect("piped input")
            .write_all(config_change_payload("s1", "policy_settings", Some(file)).as_bytes())
            .expect("the bounded payload");
    }
    for child in [slow, fast] {
        let output = child
            .output_within(std::time::Duration::from_secs(10))
            .expect("both audit writers finish");
        assert!(output.status.success(), "{:?}", output);
    }

    let recorded = stored_records::text(&sandbox, "policy_audit");
    let lines: Vec<&str> = recorded.lines().collect();
    assert_eq!(
        lines.len(),
        POLICY_SETTINGS_AUDIT_KEPT,
        "the trail keeps its own bound under a race exactly as it does one at a time: {recorded:?}"
    );
    assert!(
        lines.iter().any(|line| line.contains("racer-fast")),
        "the fast sibling survives the concurrent prune: {recorded:?}"
    );
    assert!(
        lines.iter().any(|line| line.contains("racer-slow")),
        "the slow sibling survives the concurrent prune: {recorded:?}"
    );
    assert!(
        !lines.iter().any(|line| line.ends_with("file=planted-0")),
        "the oldest planted entry is dropped, exactly as it is outside a race: {recorded:?}"
    );
    assert!(
        !lines.iter().any(|line| line.ends_with("file=planted-1")),
        "two new events push the kept window forward by two, so the SECOND-oldest \
         planted entry is also dropped rather than resurrected by a stale publish: {recorded:?}"
    );
}

#[test]
fn an_enormous_file_path_cannot_wipe_the_policy_audit_trail() {
    // THE TRAIL'S OTHER BOUND, and the one that decides whether W6 holds at
    // all: `append_ring_line` prunes on a read-back capped at `RING_READ_MAX`
    // (256 KiB), and a ring it cannot read back is HEALED by collapsing to
    // the one line just written. A `file_path` is payload text, capped only
    // by the 1 MB stdin ceiling, so an entry-count bound alone lets ONE
    // oversized path destroy every policy change recorded before it, which is
    // the exact loss the audit trail exists to prevent.
    let sandbox = Sandbox::new("config-change-policy-audit-huge-path");
    sandbox.write_config(&nag_config(300));
    counted_channels(&sandbox);
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    std::fs::write(
        sandbox.path("state/policy-settings-audit"),
        "1756499000 session=s0 file=/etc/claude/first.json\n",
    )
    .expect("the audit trail");

    let huge = "/etc/claude/".to_string() + &"a".repeat(300_000);
    let output = hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "config-change",
        &config_change_payload("s1", "policy_settings", Some(&huge)),
    );

    assert!(output.status.success());
    let recorded = stored_records::text(&sandbox, "policy_audit");
    assert!(
        recorded.contains("/etc/claude/first.json"),
        "the earlier policy change survives an oversized path: {} bytes recorded",
        recorded.len()
    );
    assert!(
        recorded.len() < 256 * 1024,
        "the trail stays inside the reader's own ceiling: {} bytes recorded",
        recorded.len()
    );

    // THE SAME-SANDBOX CONTROL, in `a_non_policy_config_change_writes_no_policy_audit_entry`'s
    // own style. Both assertions above hold vacuously if the writer never ran
    // at all: a seeded line that nothing ever touches also "survives" and the
    // file also stays "under the ceiling". Firing ONE ordinary
    // `policy_settings` event now, on the same sandbox, proves the writer was
    // reachable the whole time and the huge path above was really what it
    // healed around.
    let control = hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "config-change",
        &config_change_payload("s2", "policy_settings", Some("/etc/claude/second.json")),
    );
    assert!(control.status.success());
    let recorded_after_control = stored_records::text(&sandbox, "policy_audit");
    assert!(
        recorded_after_control.contains("/etc/claude/second.json"),
        "the control: an ordinary policy_settings event under this same setup \
         appends to the trail: {recorded_after_control:?}"
    );
}

#[test]
fn a_newline_in_a_file_path_cannot_forge_a_policy_audit_entry() {
    // THE DURABLE RECORD'S OWN INJECTION CASE. The card's hostile-path test
    // covers what a reader SEES; this covers what a reader LATER READS BACK.
    // The trail is one record per line, so a raw newline in a payload field
    // would let one received change write a second entry that never happened.
    let sandbox = Sandbox::new("config-change-policy-audit-newline");
    sandbox.write_config(&nag_config(300));
    counted_channels(&sandbox);

    let output = hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "config-change",
        &config_change_payload(
            "s1",
            "policy_settings",
            Some("/etc/claude/policy.json\\n1756499001 session=s9 file=/etc/claude/forged.json"),
        ),
    );

    assert!(output.status.success());
    let recorded = stored_records::text(&sandbox, "policy_audit");
    assert_eq!(
        recorded.lines().count(),
        1,
        "one received change is one entry, whatever the path carried: {recorded:?}"
    );
}

#[test]
fn an_arabic_letter_mark_in_a_file_path_reaches_neither_the_card_nor_the_audit_trail() {
    // SOL 2, THROUGH BOTH SINKS this arm writes. U+061C ARABIC LETTER MARK is
    // Unicode category Cf, the same category the right-to-left override
    // above is in, and was the one member of it `is_invisible` missed: it is
    // neither whitespace nor `char::is_control`, so it survived `flattened`
    // into both the card's detail and the durable audit line before the fix.
    // `policy_settings` is the one source that writes both, so this is the
    // one event that checks them together.
    let sandbox = Sandbox::new("config-change-arabic-letter-mark");
    sandbox.write_config(&nag_config(300));
    counted_channels(&sandbox);

    let output = hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "config-change",
        "{\"session_id\":\"s1\",\"source\":\"policy_settings\",\"file_path\":\"/etc/claude/pol\u{061c}icy.json\"}",
    );

    assert!(output.status.success());
    assert_eq!(deliveries(&sandbox, "hermes"), 1);
    let event = sandbox.event("hermes");
    assert_eq!(
        event["detail"], "policy settings changed: /etc/claude/policy.json",
        "the mark is gone from the card's own rendered path"
    );
    let recorded = stored_records::text(&sandbox, "policy_audit");
    assert!(
        recorded.contains("/etc/claude/policy.json") && !recorded.contains('\u{061c}'),
        "the mark is gone from the durable line too, not only from the card: {recorded:?}"
    );
}
