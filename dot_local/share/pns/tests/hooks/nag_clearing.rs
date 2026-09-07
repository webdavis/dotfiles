use super::*;

#[test]
fn an_answered_approval_is_never_nudged_by_either_clearing_signal() {
    // THE BEHAVIOR THE WHOLE FEATURE'S PROMISE RESTS ON. Two signals clear a
    // record and they go through ONE function, so there is one clearing rule
    // rather than three copies of it: `PostToolBatch` (`pns hook resolved`) is
    // the per-batch one, and Stop is the free backstop for a batch payload over
    // the 1MB cap, an operator who escaped the prompt, and the window between
    // this merge and the operator's apply.
    //
    // A STOP DELIVERS ITS OWN TURN CARD, so "delivers nothing" is asserted as
    // "the following nag adds nothing", which is the property that matters.
    for word in ["resolved", "stop"] {
        let sandbox = Sandbox::new(&format!("nag-cleared-by-{word}"));
        sandbox.write_config(&nag_config(300));
        counted_channels(&sandbox);
        write_record(&sandbox, "s1", 300, "Bash: cargo test", "wW:p21");

        let output = hook_with(
            sandbox.pns_stateful(),
            &sandbox,
            word,
            r#"{"session_id":"s1","cwd":"/a/dotfiles"}"#,
        );
        assert_eq!(output.status.code(), Some(0), "{word}");
        assert!(
            !nag_record(&sandbox, "s1").exists(),
            "{word} removes the record"
        );
        assert!(
            nag_marker(&sandbox, "s1").exists(),
            "{word} writes the marker FIRST, so a crash between the two leaves an \
             approval that is never nudged rather than one nudged after being answered"
        );

        // `resolved` DELIVERS NOTHING OF ITS OWN: it is a clearing signal on
        // every assistant tool batch this machine runs, and a hook word that
        // notified would card the operator once per batch forever. `stop`
        // legitimately reports its own turn, which is why the count is stated
        // per word rather than asserted to be zero for both.
        let expected = usize::from(word == "stop");
        assert_eq!(
            deliveries(&sandbox, "hermes"),
            expected,
            "{word}: the clearing signal itself"
        );
        support::run(&mut nag(&sandbox));
        assert_eq!(
            deliveries(&sandbox, "hermes"),
            expected,
            "{word}: a fire after the answer adds nothing at all"
        );
    }
}

#[test]
fn a_clear_landing_inside_the_fires_claim_window_still_writes_the_marker() {
    // THE WINDOW BETWEEN THE CLAIM AND THE MARKER CHECK, which is the one gap
    // the record's own presence cannot cover. The fire takes a record by
    // renaming it out of its own name, so for the length of a read, a parse and
    // a marker test there is NO `.pending` file for that session; a clear gated
    // on the record being there does nothing at all in that window, and the
    // fire then cards an approval the operator has already dealt with.
    //
    // THE MARKER IS WHAT CLOSES IT. Written unconditionally, it is on disk
    // before the holder asks, and every drop the holder can make is silence.
    let sandbox = Sandbox::new("nag-clear-inside-claim");
    sandbox.write_config(&nag_config(300));
    counted_channels(&sandbox);
    write_record(&sandbox, "s1", 300, "Bash: cargo test", "wW:p21");
    // THE FIRE'S OWN RENAME, BY HAND. The pid in the name is not read by
    // anything, so any number stands in for the process holding the claim.
    let record = nag_record(&sandbox, "s1");
    let claim = sandbox.path("state/nag/s1.pending.claim.1");
    std::fs::rename(&record, &claim).expect("the record is claimed");

    let output = hook_with(
        sandbox.pns_stateful(),
        &sandbox,
        "resolved",
        r#"{"session_id":"s1","cwd":"/a/dotfiles"}"#,
    );

    assert_eq!(output.status.code(), Some(0));
    assert!(
        nag_marker(&sandbox, "s1").exists(),
        "the answer is recorded whether or not a record is at its own name"
    );
    // AND THE HOLDER THEN DROPS IT. The record goes back to the name the
    // holding process is reading from, which is what a fire has in hand when it
    // reaches its marker check.
    std::fs::rename(&claim, &record).expect("the claim is read back");
    support::run(&mut nag(&sandbox));
    assert_eq!(
        deliveries(&sandbox, "hermes"),
        0,
        "never a nudge after an answer, whatever the answer raced"
    );
    assert!(
        !nag_record(&sandbox, "s1").exists(),
        "and the record is dropped rather than left to be re-claimed forever"
    );
}
