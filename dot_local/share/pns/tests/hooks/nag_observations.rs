use super::*;

#[test]
fn a_nudge_is_not_a_new_event() {
    // THE CONTIGUOUS TAIL OF `run_event` BELONGS TO THE FIRST DELIVERY. Each
    // line here is a defect avoided rather than tidiness: the recap counts
    // activity-ring lines toward `min_events`, so a nudge that rang would
    // inflate the operator's own recap with pns's noise; a nudge is not evidence
    // of presence, so it must not move the last-present marker; and the journal
    // is what a catch-up replays, and a "still waiting" card replayed hours
    // later about a question long since answered is worse than silence.
    let sandbox = Sandbox::new("nag-not-a-new-event");
    sandbox.write_config(&nag_config(300));
    counted_channels(&sandbox);
    let mut command = sandbox.pns_stateful();
    sandbox.stub_moshi(&mut command, 0);
    hook_with(
        command,
        &sandbox,
        "blocked",
        "{\"message\":\"Bash: cargo test\",\"session_id\":\"s1\",\"cwd\":\"/a/dotfiles\"}\n",
    );
    let ring = state_lines(&sandbox, "activity");
    let journal = state_lines(&sandbox, "missed-notifications");
    let present = std::fs::read_to_string(sandbox.path("state/last-present")).unwrap_or_default();
    assert_eq!(ring.len(), 1, "the approval's own card rang once");

    support::run(&mut nag(&sandbox));

    assert_eq!(
        state_lines(&sandbox, "activity"),
        ring,
        "a nudge writes no activity-ring line, or the recap counts pns's own noise"
    );
    assert_eq!(
        state_lines(&sandbox, "missed-notifications"),
        journal,
        "and no journal entry, so a suppressed nudge is LOST rather than replayed later"
    );
    assert_eq!(
        std::fs::read_to_string(sandbox.path("state/last-present")).unwrap_or_default(),
        present,
        "and it never claims the return moment: a nudge is not evidence of presence"
    );
}

#[test]
fn the_decision_log_says_which_line_was_the_nudge() {
    // WITHOUT THE FIELD the ring holds two `claude/blocked` lines differing in
    // nothing an operator can see, and "why did I get two cards for one prompt"
    // is the exact question this log exists to answer.
    let sandbox = Sandbox::new("nag-decision-log");
    sandbox.write_config(&nag_config(300));
    counted_channels(&sandbox);
    let mut command = sandbox.pns_stateful();
    sandbox.stub_moshi(&mut command, 0);
    hook_with(
        command,
        &sandbox,
        "blocked",
        "{\"message\":\"Bash: cargo test\",\"session_id\":\"s1\",\"cwd\":\"/a/dotfiles\"}\n",
    );
    support::run(&mut nag(&sandbox));

    let blocked: Vec<String> = state_lines(&sandbox, "decisions")
        .into_iter()
        .filter(|line| line.contains("claude/blocked"))
        .collect();
    assert_eq!(blocked.len(), 2, "one prompt, one nudge: {blocked:?}");
    assert!(
        blocked[0].contains(" nag=no "),
        "the approval's own card: {}",
        blocked[0]
    );
    assert!(
        blocked[1].contains(" nag=yes "),
        "and the nudge about it: {}",
        blocked[1]
    );
}
