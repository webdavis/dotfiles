use super::*;

#[test]
fn an_unanswered_approval_is_nudged_once_through_the_ordinary_paths() {
    let sandbox = Sandbox::new("nag-one-waiting");
    sandbox.write_config(&nag_config(300));
    counted_channels(&sandbox);
    write_record(&sandbox, "s1", 300, "Bash: cargo test", "wW:p21");

    let output = support::run(&mut nag(&sandbox));

    assert_eq!(deliveries(&sandbox, "hermes"), 1, "exactly one card");
    let event = sandbox.event("hermes");
    assert_eq!(
        event["state"], "blocked",
        "the state word stays `blocked`, or the card falls out of the recap's needs-you section"
    );
    assert_eq!(event["agent"], "claude");
    assert_eq!(event["project"], "dotfiles");
    let detail = event["detail"].as_str().expect("a detail");
    assert!(
        detail.starts_with("still waiting ") && detail.ends_with("Bash: cargo test"),
        "the card says how long and what was asked: {detail}"
    );
    assert_eq!(
        event["pane"], "wW:p21",
        "the recorded pane, so a banner click still lands on the waiting pane"
    );
    assert!(
        !nag_record(&sandbox, "s1").exists(),
        "the record is consumed by the fire that spent its one nudge"
    );
    assert!(
        nag_marker(&sandbox, "s1").exists(),
        "and the marker is what makes its own job drop silently"
    );
    // ATTEMPTED, NEVER SENT. `run_event` answers nothing about delivery, so
    // this mode cannot know whether a leg fired: a muted operator, or one
    // inside a named Focus, gets no banner and no phone card at all. The drill
    // reads this line, and bug class 19 is exactly an action reported as done
    // when it was suppressed.
    assert!(
        support::stdout(&output).contains("1 waiting; one card attempted"),
        "the fire says what it did and never more: {}",
        support::stdout(&output)
    );
    // AND THE FIRE CLEANS UP AFTER ITSELF. Its working files are the record
    // claims it held and the claim on the window itself; leaving either makes
    // one file per nudge, forever, and the accepted risk covers only what a
    // CRASH mid-fire strands.
    assert_eq!(
        nag_directory_names(&sandbox),
        Vec::<String>::new(),
        "no record claim and no fire claim outlives the fire that took them"
    );
}

#[test]
fn a_second_fire_nudges_nothing() {
    // THE ONE-NUDGE-MAXIMUM PIN: the first fire CONSUMES the record, so there
    // is nothing left for a second fire to read.
    //
    // WHAT IT DOES NOT PIN, said here because the mutation record used to claim
    // otherwise: this test does not kill the rename. A fire that read each
    // record in place and removed it afterwards, with no claim at all, passes
    // every test in this suite (measured by two reviewers independently). The
    // rename is what arbitrates between processes, and a suite of
    // single-process fires cannot see it. The property that IS pinned by a test
    // is the fire-window claim above it, which is where two processes are
    // actually run.
    let sandbox = Sandbox::new("nag-second-fire");
    sandbox.write_config(&nag_config(300));
    counted_channels(&sandbox);
    write_record(&sandbox, "s1", 300, "Bash: cargo test", "wW:p21");

    support::run(&mut nag(&sandbox));
    assert_eq!(deliveries(&sandbox, "hermes"), 1);

    let second = support::run(&mut nag(&sandbox));
    assert_eq!(
        deliveries(&sandbox, "hermes"),
        1,
        "one approval earns exactly one nudge, whatever fires afterwards"
    );
    assert!(support::stdout(&second).contains("nothing is waiting"));
}

#[test]
fn three_unanswered_approvals_produce_one_card_that_says_three() {
    // THE OPERATOR'S COALESCING RULING. A per-record loop delivers three cards,
    // which is precisely the stacking this forbids, and the three MARKERS are
    // what silence the two sibling jobs when their own timers come round.
    let sandbox = Sandbox::new("nag-coalesce");
    sandbox.write_config(&nag_config(300));
    counted_channels(&sandbox);
    write_record(&sandbox, "s1", 300, "Bash: cargo test", "wW:p21");
    // 480 RATHER THAN 600, and the slack is the point twice over. The staleness
    // cap is 2 * after_secs = 600, so a record armed at exactly the cap goes
    // STALE if the fire's own clock read lands one second later than the
    // fixture's, which under a loaded parallel suite it does. And 480 through
    // 539 all read "8m", so the sentence does not turn over either.
    write_record(&sandbox, "s2", 480, "Write: config.toml", "wW:p22");
    write_record(&sandbox, "s3", 120, "Edit: main.rs", "wW:p23");
    // A FOURTH RECORD THAT IS ALREADY ANSWERED, so the fire ENUMERATES four and
    // CLAIMS three. Without it the two counts agree and a card built from the
    // wrong one reads correctly by accident; with it, a count taken off the
    // directory listing says four and lies to the operator about how many
    // questions are actually waiting.
    write_record(&sandbox, "s4", 240, "Read: secrets", "wW:p24");
    write_marker(&sandbox, "s4");

    support::run(&mut nag(&sandbox));

    assert_eq!(deliveries(&sandbox, "hermes"), 1, "one card, never three");
    let detail = sandbox.event("hermes")["detail"]
        .as_str()
        .expect("a detail")
        .to_string();
    assert_eq!(detail, "3 approvals waiting, oldest 8m");
    for question in ["cargo test", "config.toml", "main.rs"] {
        assert!(
            !detail.contains(question),
            "a coalesced card names no single question: {detail}"
        );
    }
    for session in ["s1", "s2", "s3"] {
        assert!(
            !nag_record(&sandbox, session).exists(),
            "{session}'s record is consumed by the one card that counted it"
        );
        assert!(
            nag_marker(&sandbox, session).exists(),
            "{session}'s marker is what makes its own job drop silently"
        );
    }
    assert!(
        !nag_record(&sandbox, "s4").exists(),
        "and the answered one is dropped rather than counted"
    );
}

/// How many records the racing fires are given to split between them.
///
/// MANY AND NOT THREE, deliberately. The split needs a second process to start
/// INSIDE the first one's claim loop, and a loop over three records is finished
/// before a second process has finished exec, so a three-record race reports
/// green against a build that splits.
const RACED_RECORDS: usize = 200;

/// How many fires are started at once. Two is the case the daemon actually
/// produces (two approvals armed in one wall-clock second, both due in one
/// tick); four makes the pre-fix split near certain without making the test
/// slow.
const RACED_FIRES: usize = 4;

#[test]
fn fires_racing_over_one_directory_still_produce_exactly_one_card() {
    // THE COALESCING RULING UNDER CONCURRENCY, which the per-record claim does
    // NOT deliver on its own and never did. Two approvals armed inside one
    // wall-clock second come due in one daemon tick; the daemon spawns both and
    // waits for neither. Ownership taken PER RECORD then lets each process win
    // a disjoint, non-empty subset and card its own true count, so the operator
    // gets one card per fire rather than one card per fire window. Measured on
    // this base before the fire lock: sixteen concurrent fires over one
    // directory produced sixteen cards.
    let sandbox = Sandbox::new("nag-racing-fires");
    sandbox.write_config(&nag_config(300));
    counted_channels(&sandbox);
    for index in 0..RACED_RECORDS {
        write_record(
            &sandbox,
            &format!("s{index}"),
            300,
            "Bash: cargo test",
            "wW:p21",
        );
    }

    let fires: Vec<std::process::Child> = (0..RACED_FIRES)
        .map(|_| {
            nag(&sandbox)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .expect("a fire starts")
        })
        .collect();
    for mut fire in fires {
        assert!(
            fire.wait().expect("a fire ends").success(),
            "a fire that loses the window is not a failure: it exits 0 and says nothing"
        );
    }

    assert_eq!(
        deliveries(&sandbox, "hermes"),
        1,
        "one fire owns the whole window and the losers deliver nothing at all"
    );
    // AND EVERY RECORD IS STILL CONSUMED, so the losers standing down costs no
    // approval its nudge: the winner enumerated the directory after it owned it.
    assert_eq!(
        nag_directory_names(&sandbox)
            .iter()
            .filter(|name| name.ends_with(".pending"))
            .count(),
        0,
        "the winner counted every record, so none is left for a later fire"
    );
}

#[test]
fn the_daemon_really_fires_the_nag_and_really_drops_it_when_the_marker_is_there() {
    // THE TEST THAT PROVES THE FEATURE EXISTS END TO END: a real `pns daemon
    // run` at its fast tick, a real spool entry, a real spawned `pns nag`. The
    // second row is the only place `unless_marker` is PROVEN rather than
    // assumed, and it is what makes coalescing quiet: every sibling job of a
    // coalesced card drops through exactly this path.
    for (case, answered) in [("nobody answered", false), ("the marker is there", true)] {
        let sandbox = Sandbox::new(&format!("nag-daemon-{}", case.replace(' ', "-")));
        sandbox.write_config(&nag_config(300));
        counted_channels(&sandbox);
        write_record(&sandbox, "s1", 300, "Bash: cargo test", "wW:p21");
        if answered {
            write_marker(&sandbox, "s1");
        }
        support::run(sandbox.pns_stateful().args([
            "daemon",
            "schedule",
            "--id",
            "nag:s1",
            "--in",
            "0",
            "--unless-marker",
            "nag-s1",
            "--",
            "nag",
        ]));

        let daemon = support::DaemonGuard::start(&sandbox, 50);

        if answered {
            // A DROP IS STILL SAID, so the daemon's own line stays the probe on
            // this row: refusing a job is news, and it is the one thing this
            // case exists to prove.
            let said = support::poll_until(|| {
                let said = daemon.said();
                said.contains("nag:s1").then_some(said)
            })
            .unwrap_or_else(|| {
                panic!(
                    "{case}: the daemon never reached the job: {}",
                    daemon.said()
                )
            });
            assert!(
                said.contains("dropped `nag:s1` because its marker was already there"),
                "{case}: {said}"
            );
            // AND NOTHING WAS SPAWNED AT ALL, which the absent card is what
            // proves: a firing that worked now says nothing, so there is no
            // `ran` line left whose absence could stand for it.
            assert_eq!(
                deliveries(&sandbox, "hermes"),
                0,
                "{case}: the job is dropped WITHOUT spawning a nag process at all"
            );
        } else {
            // THE CARD IS THE PROBE, because a firing that WORKED says nothing.
            // The daemon's success line went when one line per firing became
            // 300 an hour for the lights tick, so the delivery and the consumed
            // record carry the whole path that line used to stand for, and they
            // carry more of it than the line did.
            let delivered = support::poll_until(|| {
                (deliveries(&sandbox, "hermes") > 0).then(|| deliveries(&sandbox, "hermes"))
            });
            assert_eq!(
                delivered,
                Some(1),
                "{case}: exactly one card; the daemon said: {}",
                daemon.said()
            );
            assert!(
                !nag_record(&sandbox, "s1").exists(),
                "{case}: and the record is consumed by the fire the daemon started"
            );
        }
    }
}
