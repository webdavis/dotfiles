use super::*;

#[test]
fn racing_present_events_deliver_exactly_one_replay_between_them() {
    // THE CLAIM IS A RENAME BECAUSE OF THIS. Two events firing at once is
    // ordinary here (a Stop hook and the long-running notifier are a normal
    // pair), and a read-then-remove hands the same batch to every one of them:
    // the operator gets the same missed notifications over and over.
    let sandbox = Sandbox::new("replay-race");
    record_every_event(&sandbox);
    sandbox.write_config(&recap_switched_off());
    std::fs::write(journal_path(&sandbox), planted_journal(2)).expect("the journal");

    // EVERY COMMAND IS BUILT BEFORE THE FIRST SPAWN: building one WRITES the
    // herdr stub, and rewriting a script another racer is already executing is
    // a flake rather than the race under test.
    let mut commands: Vec<std::process::Command> =
        (0..RACERS).map(|_| present_event(&sandbox)).collect();
    let racers: Vec<std::process::Child> = commands
        .iter_mut()
        .map(|command| {
            command
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .expect("the engine starts")
        })
        .collect();
    for mut racer in racers {
        // BOUNDED, so a wedged racer fails this test rather than parking the
        // suite behind it.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while racer.try_wait().expect("the child is waitable").is_none() {
            assert!(std::time::Instant::now() < deadline, "a racer never exited");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let done = racer.wait_with_output().expect("the child is waitable");
        assert!(done.status.success(), "a racer failed: {}", stderr(&done));
        assert_eq!(stdout(&done), "", "a racer printed a line");
    }

    // THE DESK ROW EARNS THE BANNER AND NOT THE CARD, so the two legs below
    // are what every racer reached and the phone is the control.
    for channel in ["macos-banner", "hermes"] {
        let delivered = events(&sandbox, channel);
        assert_eq!(
            delivered.len(),
            RACERS + 1,
            "{channel} saw something other than {RACERS} live events and one replay: {delivered:?}"
        );
        assert_eq!(
            delivered
                .iter()
                .filter(|event| event["state"] == "missed")
                .count(),
            1,
            "{channel} was handed the one batch more than once: {delivered:?}"
        );
    }
    assert!(
        events(&sandbox, "mobile").is_empty(),
        "the phone is not a leg for an operator at the desk"
    );
    stored_records::assert_consumed(&sandbox);
}

/// A SOAK, NOT A GATE, which is why it is ignored by default: run it with
/// `cargo test -- --ignored --exact` and a loop around it. On the build that
/// owned a claim by unlinking it, one round in 200 caught the double, and
/// raising the racer count did not improve that (the spawn spread grows with
/// the pair count). The deterministic statements of this invariant are the two
/// held-file tests above; this one is corroboration, and each racer is bounded
/// so a wedged one fails rather than parks the suite.
#[test]
#[ignore = "soak: a probabilistic hunt, roughly one catch in 200 rounds"]
fn racing_present_events_adopt_one_stranded_claim_exactly_once() {
    // ONE STRANDED CLAIM AND NO JOURNAL, which puts every racer on the SAME
    // adoption path at the same moment: the rename that arbitrates the journal
    // never runs, so all that stands between the batch and N deliveries is
    // whatever `take_claim` uses to decide ownership.
    let sandbox = Sandbox::new("replay-adopt-race");
    record_every_event(&sandbox);
    sandbox.write_config(&recap_switched_off());
    let stranded = journal_path(&sandbox).with_extension("claim.999999");
    std::fs::write(&stranded, planted_journal(2)).expect("the stranded claim");

    let mut commands: Vec<std::process::Command> =
        (0..ADOPTERS).map(|_| present_event(&sandbox)).collect();
    let racers: Vec<std::process::Child> = commands
        .iter_mut()
        .map(|command| {
            command
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .expect("the engine starts")
        })
        .collect();
    for mut racer in racers {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while racer.try_wait().expect("the child is waitable").is_none() {
            assert!(std::time::Instant::now() < deadline, "a racer never exited");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let done = racer.wait_with_output().expect("the child is waitable");
        assert!(done.status.success(), "a racer failed: {}", stderr(&done));
    }

    for channel in ["macos-banner", "hermes"] {
        let delivered = events(&sandbox, channel);
        assert_eq!(
            delivered
                .iter()
                .filter(|event| event["state"] == "missed")
                .count(),
            1,
            "{channel} was handed the one stranded batch more than once: {delivered:?}"
        );
    }
    stored_records::assert_consumed(&sandbox);
}
