use super::*;

#[test]
fn a_record_whose_marker_appeared_is_dropped_rather_than_nudged() {
    // THE RACE BETWEEN AN ANSWER AND A WAKING DAEMON. The marker was written
    // while this process was starting, so the approval is already resolved and
    // the safe direction is silence.
    let sandbox = Sandbox::new("nag-marker-race");
    sandbox.write_config(&nag_config(300));
    counted_channels(&sandbox);
    write_record(&sandbox, "s1", 300, "Bash: cargo test", "wW:p21");
    write_marker(&sandbox, "s1");

    let output = support::run(&mut nag(&sandbox));

    assert_eq!(
        deliveries(&sandbox, "hermes"),
        0,
        "never a nudge after an answer"
    );
    assert!(
        !nag_record(&sandbox, "s1").exists(),
        "and the record is cleared"
    );
    assert!(support::stdout(&output).contains("nothing is waiting"));
}

#[test]
fn a_fire_with_the_feature_switched_off_drops_every_record_and_cards_nothing() {
    // THE OPERATOR CANCELLED THE TIMER between the arming and the fire, so a
    // card from it would be the feature ignoring them. THE RECORDS GO TOO: one
    // left behind is a card waiting to be delivered the moment they switch the
    // feature back on, about a prompt from whenever it was.
    let sandbox = Sandbox::new("nag-off-drops-records");
    sandbox.write_config(&nag_config(0));
    counted_channels(&sandbox);
    write_record(&sandbox, "s1", 300, "Bash: cargo test", "wW:p21");

    let output = support::run(&mut nag(&sandbox));

    assert_eq!(
        deliveries(&sandbox, "hermes"),
        0,
        "a cancelled timer delivers nothing"
    );
    assert!(
        support::stdout(&output).contains("the nag is off; 1 waiting approval(s) dropped"),
        "and it says what it dropped: {}",
        support::stdout(&output)
    );
    assert!(
        !nag_record(&sandbox, "s1").exists(),
        "the record goes with the timer that was cancelled"
    );
}

#[test]
fn a_record_whose_name_is_not_a_session_is_dropped_loudly_rather_than_re_read_forever() {
    // THE UNREADABLE-CONTENT CASE IS DROPPED WITH A LINE ON STDERR, on the
    // stated reasoning that dropping one in silence is how a file sits there
    // being re-claimed on every fire forever. A record whose NAME cannot be a
    // session id has exactly that property and was handled the opposite way: a
    // bare skip, never claimed, never removed, never mentioned.
    let sandbox = Sandbox::new("nag-unusable-name");
    sandbox.write_config(&nag_config(300));
    counted_channels(&sandbox);
    // A FILE WHOSE WHOLE NAME IS THE SUFFIX. Stripping it leaves an empty
    // session id, which is not a name a record, a marker or a job can be
    // written for, so nothing about this file can be acted on.
    let stray = sandbox.path("state/nag/.pending");
    std::fs::create_dir_all(stray.parent().expect("the nag directory")).expect("the nag directory");
    std::fs::write(&stray, "{}").expect("the stray file");

    let output = support::run(&mut nag(&sandbox));

    assert!(
        support::stderr(&output).contains(".pending"),
        "the file is NAMED, so an operator can see what was dropped: {}",
        support::stderr(&output)
    );
    assert_eq!(
        deliveries(&sandbox, "hermes"),
        0,
        "and nothing is carded about a record nothing can be resolved from"
    );
    assert_eq!(
        nag_directory_names(&sandbox),
        Vec::<String>::new(),
        "dropped ONCE: it is gone rather than left to be re-read on every fire"
    );
}

#[test]
fn pns_nag_refuses_an_argument_rather_than_falling_through_to_a_fire() {
    // THE HOUSE RULE: an unknown argument never falls through to help with exit
    // 0. `pns nag <session>` is a command an operator would believe narrowed
    // the fire to one approval, and coalescing means nothing here can honour
    // it, so it is refused rather than silently ignored.
    let sandbox = Sandbox::new("nag-usage");
    sandbox.write_config(&nag_config(300));
    counted_channels(&sandbox);
    write_record(&sandbox, "s1", 300, "Bash: cargo test", "wW:p21");

    let output = nag(&sandbox).arg("s1").output().expect("the engine runs");

    assert_eq!(output.status.code(), Some(2), "a refusal, never exit 0");
    assert!(
        support::stderr(&output).contains("it takes no arguments"),
        "and the usage is on stderr: {}",
        support::stderr(&output)
    );
    assert_eq!(
        deliveries(&sandbox, "hermes"),
        0,
        "a refused command delivers nothing"
    );
    assert!(
        nag_record(&sandbox, "s1").exists(),
        "and consumes nothing either: the approval is still waiting"
    );
}

#[test]
fn a_stale_record_is_dropped_rather_than_nudged() {
    // BOTH SIDES OF THE CAP, in one fire. The first row is the card that wakes
    // a laptop to describe last night's prompt; the second is bug class 2, a
    // clock that moved backwards or a hand-edited epoch, which a one-sided
    // implementation would read as fresh forever.
    let sandbox = Sandbox::new("nag-stale");
    sandbox.write_config(&nag_config(300));
    counted_channels(&sandbox);
    write_record(&sandbox, "old", 7_200, "Bash: last night", "wW:p21");
    write_record_at(
        &sandbox,
        "ahead",
        epoch_now() + 3_600,
        "Bash: tomorrow",
        "wW:p22",
    );

    let output = support::run(&mut nag(&sandbox));

    assert_eq!(deliveries(&sandbox, "hermes"), 0, "neither row is news");
    for session in ["old", "ahead"] {
        assert!(
            !nag_record(&sandbox, session).exists(),
            "{session}'s record is dropped rather than left to be re-claimed forever"
        );
    }
    assert!(support::stdout(&output).contains("nothing is waiting"));
}
