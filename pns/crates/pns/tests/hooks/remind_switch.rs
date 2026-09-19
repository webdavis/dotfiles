use super::*;

/// The stub channels with a delay set and NO producer entry at all, which is
/// the default posture: the reminder is off until something switches it on.
fn delay_only(delay_secs: u64) -> String {
    remind_config(delay_secs).replace("[producer.claude]\nremind = true\n", "")
}

/// One blocked approval with the switch this call carried.
fn blocked_with(sandbox: &Sandbox, flags: &[&str]) -> std::process::Output {
    let mut command = sandbox.pns_stateful();
    sandbox.stub_moshi(&mut command, 0);
    hook_flagged(
        command,
        "blocked",
        flags,
        "{\"message\":\"Bash: cargo test\",\"session_id\":\"s1\"}\n",
    )
}

/// How long the nudge this run registered waits, off the spool entry's own
/// due and the record's armed second.
fn armed_delay(sandbox: &Sandbox) -> u64 {
    let entry = spool_entry(sandbox, "s1");
    let due: u64 = entry
        .split('\t')
        .find_map(|part| part.strip_prefix("due="))
        .unwrap_or_else(|| panic!("no `due=` in {entry}"))
        .parse()
        .expect("a due second");
    let record = std::fs::read_to_string(remind_record(sandbox, "s1")).expect("a record");
    let parsed: serde_json::Value = serde_json::from_str(&record).expect("the record is JSON");
    due - parsed["armed"].as_u64().expect("an armed second")
}

#[test]
fn the_call_switch_arms_a_reminder_the_producer_table_never_asked_for() {
    // THE DEFAULT IS OFF AND THE FLAG IS WHAT BEATS IT. Nothing about the
    // producer's name reaches this decision any more, so a config with a delay
    // and no entry arms nothing until the call says so.
    let sandbox = Sandbox::new("remind-switch-flag-over-default");
    sandbox.write_config(&delay_only(300));

    assert_eq!(blocked_with(&sandbox, &[]).status.code(), Some(0));
    assert!(
        !remind_record(&sandbox, "s1").exists(),
        "a delay alone arms nothing"
    );

    assert_eq!(blocked_with(&sandbox, &["--remind"]).status.code(), Some(0));
    assert!(
        remind_record(&sandbox, "s1").exists(),
        "and `--remind` is what arms it"
    );
    assert_eq!(armed_delay(&sandbox), 300, "at the configured delay");
}

#[test]
fn the_calls_own_duration_beats_the_configured_delay() {
    let sandbox = Sandbox::new("remind-switch-duration");
    sandbox.write_config(&delay_only(300));

    assert_eq!(
        blocked_with(&sandbox, &["--remind=45s"]).status.code(),
        Some(0)
    );
    assert_eq!(armed_delay(&sandbox), 45);
}

#[test]
fn a_space_separated_duration_is_never_read_as_the_delay() {
    // `--remind 45s` IS `--remind` PLUS A STRAY WORD, the convention of `git
    // log --color[=<when>]`: the next token is never swallowed, so a flag
    // standing behind one is still its own flag.
    let sandbox = Sandbox::new("remind-switch-spaced");
    sandbox.write_config(&delay_only(300));

    assert_eq!(
        blocked_with(&sandbox, &["--remind", "45s"]).status.code(),
        Some(0)
    );
    assert_eq!(armed_delay(&sandbox), 300);
}

#[test]
fn the_call_can_disarm_a_reminder_the_producer_table_asked_for() {
    let sandbox = Sandbox::new("remind-switch-no-remind");
    sandbox.write_config(&remind_config(300));

    assert_eq!(
        blocked_with(&sandbox, &["--no-remind"]).status.code(),
        Some(0)
    );
    assert!(
        !remind_record(&sandbox, "s1").exists(),
        "`--no-remind` beats the producer's own entry"
    );
    assert!(
        spool_entries(&sandbox)
            .iter()
            .all(|entry| !entry.starts_with("remind:")),
        "and registers no job either"
    );
}

#[test]
fn a_switch_with_no_delay_to_run_at_is_refused_and_names_both_fixes() {
    // EXIT 2 AND NOTHING DELIVERED: this is argv the caller typed wrong, not a
    // notification that failed, and guessing a delay is the one answer a
    // reminder must not give.
    let sandbox = Sandbox::new("remind-switch-no-delay");
    sandbox.write_config(&delay_only(0));

    let output = blocked_with(&sandbox, &["--remind"]);
    assert_eq!(output.status.code(), Some(2));
    let said = support::stderr(&output);
    assert!(
        said.contains("[remind]") && said.contains("delay"),
        "the config fix is named: {said}"
    );
    assert!(
        said.contains("--remind=<duration>"),
        "and so is the per-call one: {said}"
    );
    assert_eq!(
        support::stdout(&output),
        "",
        "stdout is moshi's alone, whatever this refused: {output:?}"
    );
    assert!(!remind_record(&sandbox, "s1").exists());
}

#[test]
fn a_duration_outside_the_configured_range_is_refused_by_the_same_rule() {
    let sandbox = Sandbox::new("remind-switch-bad-duration");
    sandbox.write_config(&delay_only(300));

    for value in ["--remind=300", "--remind=1s", "--remind=9h"] {
        let output = blocked_with(&sandbox, &[value]);
        assert_eq!(output.status.code(), Some(2), "{value}");
        assert!(
            support::stderr(&output).contains("--remind"),
            "{value}: the refusal names the flag: {}",
            support::stderr(&output)
        );
    }
}
