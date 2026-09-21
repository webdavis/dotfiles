use super::*;

/// The stub channels with a delay set and NO producer entry, which is what a
/// harness that passes `--remind` on its own approval hook runs against.
fn delay_only(delay_secs: u64) -> String {
    remind_config(delay_secs).replace("[producer.claude]\nremind = true\n", "")
}

/// One blocked approval, with whatever switch the call carried.
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

/// Age the armed reminder past the staleness cap (twice the delay), fire, and
/// answer how many cards the fire itself added: the approval that armed it
/// carded the operator already, so the count before the fire is the baseline.
fn cards_from_a_fire_past_the_cap(sandbox: &Sandbox) -> usize {
    let carded = deliveries(sandbox, "hermes");
    write_record(sandbox, "s1", 7_200, "Bash: cargo test", "wW:p21");
    support::run(&mut remind(sandbox));
    deliveries(sandbox, "hermes") - carded
}

#[test]
fn a_producer_that_sends_no_answered_signal_is_warned_about_on_stderr_alone() {
    // ARMED FROM CONFIG IS ARMED BY NOBODY WHO ANSWERS. `[producer.<name>]
    // remind` exists for a producer that cannot be passed a flag, so no
    // harness has claimed to send the signal that clears the record, and the
    // cap is all that ends the nudge.
    //
    // ON STDERR, AND THE ASSERTION ON STDOUT IS THE POINT OF THE SLICE: Claude
    // Code parses this hook's stdout as moshi's decision object, so one warning
    // line there turns an Allow into no decision at all.
    let sandbox = Sandbox::new("remind-no-answered-signal");
    sandbox.write_config(&remind_config(300));
    counted_channels(&sandbox);

    let output = blocked_with(&sandbox, &[]);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        support::stdout(&output),
        "",
        "not one byte reaches the harness's parser: {output:?}"
    );
    let said = support::stderr(&output);
    assert_eq!(
        said.lines().collect::<Vec<_>>(),
        [
            "pns: `claude` sends no answered signal, so this reminder is stopped only by the \
          `[remind]` staleness cap"
        ],
        "exactly one line, naming the producer and what will stop the nudge"
    );
    assert!(
        remind_record(&sandbox, "s1").exists(),
        "and the reminder is armed anyway: the warning says what will happen, not that \
         nothing did"
    );

    assert_eq!(
        cards_from_a_fire_past_the_cap(&sandbox),
        0,
        "the staleness cap is still the hard stop"
    );
    assert!(
        !remind_record(&sandbox, "s1").exists(),
        "and the stale record is dropped rather than re-claimed forever"
    );
}

#[test]
fn the_calls_own_switch_is_the_answered_signal_and_is_warned_about_nowhere() {
    // A HARNESS WIRES `--remind` ON ITS APPROVAL HOOK ONLY WHEN IT ALSO WIRES
    // THE ANSWERED EVENT, so the switch is the assertion pns has no other way
    // to get, and a line on stderr for every approval would be noise.
    let sandbox = Sandbox::new("remind-answered-signal");
    sandbox.write_config(&delay_only(300));
    counted_channels(&sandbox);

    let output = blocked_with(&sandbox, &["--remind"]);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(support::stderr(&output), "", "nothing is said: {output:?}");
    assert!(remind_record(&sandbox, "s1").exists(), "and it is armed");

    assert_eq!(
        cards_from_a_fire_past_the_cap(&sandbox),
        0,
        "the cap bounds this arm too, answered signal or not"
    );
    assert!(!remind_record(&sandbox, "s1").exists());
}
