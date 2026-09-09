use super::*;

#[test]
fn arming_writes_a_record_registers_a_job_and_clears_a_stale_marker_first() {
    let sandbox = Sandbox::new("nag-arms");
    sandbox.write_config(&nag_config(300));
    let mut command = sandbox.pns_stateful();
    sandbox.stub_moshi(&mut command, 0);
    // THE STALE MARKER IS BUG CLASS 14 wearing this feature's clothes: the
    // marker name is constant PER SESSION, so one left by the previous approval
    // in this session would make the NEW job drop silently and the new approval
    // would never be nudged. Identity is not presence.
    write_marker(&sandbox, "s1");

    let output = hook_with(
        command,
        &sandbox,
        "blocked",
        "{\"message\":\"Bash: cargo test\",\"session_id\":\"s1\"}\n",
    );
    assert_eq!(output.status.code(), Some(0));

    let record = nag_record(&sandbox, "s1");
    let raw = std::fs::read_to_string(&record).expect("a record");
    let parsed: serde_json::Value = serde_json::from_str(&raw).expect("the record is JSON");
    assert_eq!(parsed["detail"], "Bash: cargo test");
    assert_eq!(parsed["agent"], "claude");
    assert_eq!(
        std::os::unix::fs::PermissionsExt::mode(
            &std::fs::metadata(&record)
                .expect("the record")
                .permissions()
        ) & 0o777,
        0o600,
        "state is owner-only, like every other file this crate publishes"
    );

    let entry = spool_entry(&sandbox, "s1");
    assert!(entry.contains("id=nag:s1"), "one job per approval: {entry}");
    assert!(
        entry.contains("marker=nag-s1"),
        "and `unless_marker` is what silences it once the answer lands: {entry}"
    );
    assert!(
        entry.contains(r#"args=["nag"]"#),
        "the fire takes no argument, so no free text reaches the spool: {entry}"
    );
    // AND THE LEASE IS A WHOLE SCHEDULE PAST THE DUE SECOND. `until == due` is
    // a zero-length lease: the daemon drops a job one second past `until`, so a
    // busy tick or a laptop that woke a moment late loses the nudge entirely,
    // and nothing about the card that never arrived says why.
    let field = |key: &str| -> u64 {
        entry
            .split('\t')
            .find_map(|part| part.strip_prefix(key))
            .unwrap_or_else(|| panic!("no `{key}` in {entry}"))
            .parse()
            .expect("a count")
    };
    assert_eq!(
        field("until=") - field("due="),
        300,
        "the lease runs one more schedule past the due second: {entry}"
    );

    assert!(
        !nag_marker(&sandbox, "s1").exists(),
        "the marker left by this session's PREVIOUS approval is cleared, or this one is \
         silently never nudged"
    );
    // THE SINGLE-SUBMITTER RULE, asserted here rather than in a twentieth test:
    // arming must not add a second round trip to moshi.
    assert_eq!(submissions(&sandbox), vec!["claude-hook".to_string()]);
}

#[test]
fn an_approval_whose_nudge_could_not_be_scheduled_leaves_no_record_behind() {
    // THE STDERR LINE SAYS "this approval will not be nudged", and a record
    // left enumerable makes that untrue: no job exists to wake a fire for it,
    // but any SIBLING approval's fire, and the operator's own `pns nag`, counts
    // it and cards. Bug class 19 the other way round, a stated fact the state
    // on disk contradicts.
    let sandbox = Sandbox::new("nag-registration-refused");
    sandbox.write_config(&nag_config(300));
    counted_channels(&sandbox);
    let mut command = sandbox.pns_stateful();
    sandbox.stub_moshi(&mut command, 0);
    // THE REGISTRATION IS MADE TO FAIL, on behavior 17's own rig: a regular
    // file where the spool directory belongs, so the write is refused and
    // nothing can repair it.
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    std::fs::write(sandbox.path("state/daemon"), "not a directory").expect("the blocked spool");

    let output = hook_with(
        command,
        &sandbox,
        "blocked",
        "{\"message\":\"Bash: cargo test\",\"session_id\":\"s1\",\"cwd\":\"/a/dotfiles\"}\n",
    );

    assert!(
        support::stderr(&output).contains("will not be nudged"),
        "the failure is said: {}",
        support::stderr(&output)
    );
    assert!(
        !nag_record(&sandbox, "s1").exists(),
        "and the record goes with it, or the sentence above is false"
    );
    let delivered = deliveries(&sandbox, "hermes");
    support::run(&mut nag(&sandbox));
    assert_eq!(
        deliveries(&sandbox, "hermes"),
        delivered,
        "a fire finds nothing to count, which is what `will not be nudged` means"
    );
}

#[test]
fn nothing_is_armed_when_nothing_should_be() {
    // WRITTEN SECOND WITHIN ITS GROUP, DELIBERATELY. It is red only against the
    // obvious over-implementation (arming unconditionally), which is exactly
    // the mutation it exists to kill.
    for (case, config, agent) in [
        (
            "no [nag] table at all",
            nag_config(300).replace("[nag]\nafter_secs = 300\n", ""),
            "claude",
        ),
        ("the schedule switched off", nag_config(0), "claude"),
        // NO NAG ON CODEX. Codex wires exactly Stop and PermissionRequest, so
        // it has a turn-end clear and no batch-level one, and a Codex nag would
        // fire on every approval whose turn outlives the schedule. Agent turns
        // in this repo routinely run tens of minutes, so that is the common
        // case rather than an edge.
        ("the agent is codex", nag_config(300), "codex"),
    ] {
        let sandbox = Sandbox::new(&format!("nag-unarmed-{}", case.replace(' ', "-")));
        sandbox.write_config(&config);
        let mut command = sandbox.pns_stateful();
        sandbox.stub_moshi(&mut command, 0);
        command.env("PNS_AGENT", agent);

        hook_with(
            command,
            &sandbox,
            "blocked",
            "{\"message\":\"Bash: cargo test\",\"session_id\":\"s1\"}\n",
        );

        assert!(
            !nag_record(&sandbox, "s1").exists(),
            "{case}: no record is written"
        );
        assert!(
            spool_entries(&sandbox).is_empty(),
            "{case}: and no job is registered"
        );
    }
}

#[test]
fn arming_writes_nothing_the_harness_could_read_as_a_decision() {
    // A GUARD, NOT A RED-FIRST BEHAVIOR: it passes on the first commit and its
    // job is to keep passing. Included over the usual objection because the
    // failure is SILENT and lands in somebody else's system: Claude Code parses
    // this hook's stdout as `if (!trimmed.startsWith("{")) return { plainText }`,
    // so one stray line in front of moshi's object turns an Allow into no
    // decision at all, and the operator's approval simply evaporates.
    let sandbox = Sandbox::new("nag-arm-stdout");
    sandbox.write_config(&nag_config(300));
    let mut command = sandbox.pns_stateful();
    sandbox.stub_moshi(&mut command, 42);
    // THE REGISTRATION IS MADE TO FAIL: a regular file where the spool
    // directory belongs, so the write is refused and nothing can be repaired.
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    std::fs::write(sandbox.path("state/daemon"), "not a directory").expect("the blocked spool");

    let output = hook_with(
        command,
        &sandbox,
        "blocked",
        "{\"message\":\"Bash: cargo test\",\"session_id\":\"s1\"}\n",
    );

    assert_eq!(
        output.status.code(),
        Some(42),
        "the exit code is moshi's decision, unchanged by anything the nag did"
    );
    assert_eq!(
        support::stdout(&output),
        "",
        "the moshi stub printed nothing, so the hook must print nothing: {output:?}"
    );
    assert!(
        support::stderr(&output).contains("will not be nudged"),
        "and the failure is SAID, on stderr: an action that suppressed its own error \
         has only been attempted"
    );
}
