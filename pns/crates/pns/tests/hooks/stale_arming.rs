use super::*;

/// The config the escalation is armed under: the shipped window, written out.
fn stale_config(stale_after_secs: u64) -> String {
    format!(
        "{}[nag]\nafter_secs = 0\nstale_after_secs = {stale_after_secs}\n",
        support::STUB_CHANNELS
    )
}

#[test]
fn a_blocked_approval_arms_one_leased_escalation_job_for_every_harness() {
    // FOR EVERY HARNESS, where the nag arms for claude alone. That gate exists
    // because a five-minute nudge would be wrong in the common case for a
    // Codex turn that runs tens of minutes; an hour is past any normal turn,
    // so the gate does not carry over.
    for agent in ["claude", "codex"] {
        let sandbox = Sandbox::new(&format!("stale-arms-{agent}"));
        sandbox.write_config(&stale_config(3600));
        let mut command = sandbox.pns_stateful();
        sandbox.stub_moshi(&mut command, 0);
        command.env("PNS_PRODUCER", agent);

        let output = hook_with(
            command,
            &sandbox,
            "blocked",
            "{\"message\":\"Bash: cargo test\",\"session_id\":\"s1\"}\n",
        );
        assert_eq!(output.status.code(), Some(0));

        let entry = std::fs::read_to_string(sandbox.path("state/daemon/stale:s1"))
            .unwrap_or_else(|error| panic!("{agent}: no escalation job: {error}"));
        assert!(
            entry.contains("id=stale:s1"),
            "{agent}: one job per block: {entry}"
        );
        assert!(
            entry.contains(r#"args=["stale"]"#),
            "{agent}: the fire reads the row, so no free text reaches the spool: {entry}"
        );
        assert!(
            !entry.contains("marker="),
            "{agent}: the row is the authority, not the nag's answered marker: {entry}"
        );
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
            3600,
            "{agent}: the lease runs one more window past the due second: {entry}"
        );
        assert!(
            !nag_record(&sandbox, "s1").exists(),
            "{agent}: and the nudge beside it is off, so nothing armed it"
        );
    }
}

#[test]
fn an_escalation_window_of_zero_arms_nothing() {
    let sandbox = Sandbox::new("stale-unarmed");
    sandbox.write_config(&stale_config(0));
    let mut command = sandbox.pns_stateful();
    sandbox.stub_moshi(&mut command, 0);

    hook_with(
        command,
        &sandbox,
        "blocked",
        "{\"message\":\"Bash: cargo test\",\"session_id\":\"s1\"}\n",
    );

    assert!(
        spool_entries(&sandbox)
            .iter()
            .all(|entry| !entry.starts_with("stale:")),
        "zero is the feature off: {:?}",
        spool_entries(&sandbox)
    );
}

#[test]
fn the_fire_refuses_a_session_argument_and_says_nothing_when_the_window_is_off() {
    // ONE FIRE COVERS EVERY STUCK SESSION, so an argument is a value it would
    // have to ignore; the house rule is that an unknown argument is a refusal
    // rather than a silent fallthrough.
    let sandbox = Sandbox::new("stale-usage");
    sandbox.write_config(&stale_config(0));
    let refused = sandbox
        .pns_stateful()
        .args(["stale", "s1"])
        .output()
        .expect("the engine runs");
    assert_eq!(refused.status.code(), Some(2));
    assert!(
        support::stderr(&refused).contains("usage: pns stale"),
        "{}",
        support::stderr(&refused)
    );

    let off = support::run(sandbox.pns_stateful().arg("stale"));
    assert_eq!(off.status.code(), Some(0));
    assert!(
        support::stdout(&off).contains("the stale-block escalation is off"),
        "{}",
        support::stdout(&off)
    );
}

#[test]
fn a_fire_with_nothing_stuck_says_so_and_delivers_nothing() {
    let sandbox = Sandbox::new("stale-nothing-stuck");
    sandbox.write_config(&stale_config(3600));
    counted_channels(&sandbox);

    let output = support::run(sandbox.pns_stateful().arg("stale"));

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(deliveries(&sandbox, "hermes"), 0);
    assert!(
        support::stdout(&output).contains("nothing is stuck"),
        "{}",
        support::stdout(&output)
    );
}
