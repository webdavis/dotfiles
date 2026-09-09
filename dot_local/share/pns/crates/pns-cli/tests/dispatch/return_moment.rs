use super::*;

#[test]
fn a_window_claim_whose_owner_is_gone_is_adopted_rather_than_lost_or_left_behind() {
    // THE NEAR EDGE COMES OFF WHAT WAS CLAIMED, and this is the deterministic
    // shape of that. There is NO marker here at all: the only place the
    // window's near edge exists is inside a claim a killed run left behind. A
    // build that reads `last-present` before claiming it finds nothing, calls
    // that no window, and recaps nothing; a build that derives the window from
    // what it claimed recovers the edge and posts.
    //
    // AND THE LITTER GOES WITH IT. The adoption is the same pass that sweeps
    // the file, so a run killed between the rename and the cleanup cannot
    // leave one in the state directory for good.
    let sandbox = Sandbox::new("recap-adopt-claim");
    record_every_event(&sandbox);
    plant_window_claim(&sandbox, a_reaped_pid(), 3600);
    std::fs::write(activity_path(&sandbox), planted_activity(12, 1800, Some(4))).expect("the ring");
    std::fs::write(journal_path(&sandbox), planted_journal(2)).expect("the journal");

    run(&mut present_event(&sandbox));

    let raised = events(&sandbox, "macos-banner");
    assert_eq!(
        raised.len(),
        2,
        "the live event and ONE recap card off the adopted edge: {raised:?}"
    );
    let body = raised[1]["detail"].as_str().expect("a detail");
    assert!(
        body.contains("13 events"),
        "the window was not counted from the claimed edge: {body}"
    );
    stored_records::assert_consumed(&sandbox);
}

#[test]
fn an_event_inside_another_runs_return_moment_delivers_no_card_of_any_kind() {
    // NO SECOND CARD OF ANY KIND, and this is the half a racing test can only
    // measure. The holder of the moment is a LIVE process, so the marker is
    // renamed out of the way and its claim names an owner that still exists.
    // Before this, an event landing there read no window, fell through to the
    // journal, and put its catch-up card on the phone beside the holder's
    // recap card: MEASURED at roughly one run in three with eight racers.
    //
    // THE QUEUE IS UNTOUCHED, which is the assertion that says WHICH silence
    // this is. A build that stays quiet by consuming the journal and saying
    // nothing has lost the notifications rather than deferred them.
    let sandbox = Sandbox::new("recap-moment-busy");
    record_every_event(&sandbox);
    // THE TEST'S OWN PROCESS IS THE HOLDER, which is the only id a test can
    // name and be certain is alive for as long as the assertion needs it.
    plant_window_claim(&sandbox, std::process::id(), 3600);
    std::fs::write(activity_path(&sandbox), planted_activity(12, 1800, Some(4))).expect("the ring");
    std::fs::write(journal_path(&sandbox), planted_journal(2)).expect("the journal");
    let before = std::fs::read(journal_path(&sandbox)).expect("the journal");

    run(&mut present_event(&sandbox));

    let raised = events(&sandbox, "macos-banner");
    assert_eq!(
        raised.len(),
        1,
        "an event inside another run's return moment delivered a card: {raised:?}"
    );
    assert_eq!(
        raised[0]["state"], "done",
        "the live event alone: {raised:?}"
    );
    assert_eq!(
        std::fs::read(journal_path(&sandbox)).expect("the journal"),
        before,
        "the queue was consumed by a run that delivered nothing"
    );
    assert!(
        events(&sandbox, "hermes")
            .iter()
            .all(|event| event["state"] != "recap"),
        "a racer inside another run's moment posted its own recap: {:?}",
        events(&sandbox, "hermes")
    );
    // AND THE EDGE IS STILL THE HOLDER'S TO PUT BACK, which is the property
    // the silence is built on. MEASURED at one run in sixty with eight racers
    // before this held: a run that stood down here still published the marker
    // on its way out, out from under the holder, and the next run renamed that
    // fresh marker and became a SECOND owner alongside the first. The two
    // raced on the journal and delivered a card each.
    assert!(
        !sandbox.path("state/last-present").exists(),
        "a run that stood down republished the edge somebody else was holding: {:?}",
        state_files(&sandbox)
    );
}

#[test]
fn the_windows_near_edge_never_moves_backward_however_late_an_event_publishes() {
    // READ, COMPARE, PUBLISH. Two events at one moment both publish the edge
    // at the end of their own run, so a slow one that read an older clock used
    // to land last and put the edge BACK. Everything the quick event covered
    // then reads as absence activity on the next return, and a long enough
    // tail of it crosses the threshold and recaps a window that never happened.
    //
    // A MARKER AHEAD OF NOW IS THE CONSTRUCTIBLE SHAPE of that, and the same
    // rule answers it: the newer value stands, at both write sites, so a claim
    // that took a future edge puts the future edge back.
    let sandbox = Sandbox::new("recap-marker-monotonic");
    record_every_event(&sandbox);
    let ahead = epoch_now() + 3600;
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    std::fs::write(sandbox.path("state/last-present"), format!("{ahead}\n")).expect("the marker");

    run(&mut present_event(&sandbox));

    assert_eq!(
        last_present(&sandbox),
        Some(ahead),
        "this event's own older clock overwrote a newer edge"
    );
}

#[test]
fn racing_present_events_recap_one_loud_window_exactly_once_between_them() {
    // THE MOMENT IS CLAIMED BY RENAME BECAUSE OF THIS. Two events firing at
    // once is ordinary here, and every one of them counts the same loud window
    // before any of them moves the marker, so without an arbiter each would
    // card the phone and post its own copy of the same recap to Discord.
    // Publishing the marker cannot arbitrate: every racer reads the old value
    // first. Only one rename can win.
    let sandbox = Sandbox::new("recap-race");
    record_every_event(&sandbox);
    loud_window(&sandbox);

    // EVERY COMMAND IS BUILT BEFORE THE FIRST SPAWN, for the reason the
    // journal's own race test states: building one WRITES the herdr stub.
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
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while racer.try_wait().expect("the child is waitable").is_none() {
            assert!(std::time::Instant::now() < deadline, "a racer never exited");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let done = racer.wait_with_output().expect("the child is waitable");
        assert!(done.status.success(), "a racer failed: {}", stderr(&done));
    }

    // THE DISCORD HALF FIRST, polled because the recap is written by a process
    // none of the racers waited for, and asserted BEFORE the card so a run
    // that posted twice says so rather than being reported as a card problem.
    poll_until(|| {
        events(&sandbox, "hermes")
            .into_iter()
            .find(|event| event["state"] == "recap")
    })
    .unwrap_or_else(|| panic!("no racer posted a recap at all"));
    assert_eq!(
        events(&sandbox, "hermes")
            .iter()
            .filter(|event| event["state"] == "recap")
            .count(),
        1,
        "one window was recapped more than once: {:?}",
        events(&sandbox, "hermes")
    );
    // AND ONE CARD OF ANY KIND, which is the assertion this test used to be
    // unable to make. Counting only recap-shaped cards let the OTHER
    // duplicate through: a racer that found the marker held read no window,
    // fell through to the journal, and delivered its catch-up card beside the
    // winner's recap card, one run in three. Eight live events plus exactly
    // one card is the whole permitted output.
    //
    // AND NO SECOND RECAP IS REACHABLE HERE BY ARITHMETIC, not by luck: the
    // winner's own activity entry is stamped at exactly the edge it restores,
    // and the near edge is exclusive, so a later window can hold at most the
    // seven other racers, one under the threshold.
    let raised = events(&sandbox, "macos-banner");
    assert_eq!(
        raised.len(),
        RACERS + 1,
        "eight live events and ONE card between them: {raised:?}"
    );
    assert_eq!(
        raised
            .iter()
            .filter(|event| event["state"] == "missed")
            .count(),
        1,
        "one return moment delivered more than one card: {raised:?}"
    );
    // AND NOTHING IS HOLDING THE WINDOW AFTERWARDS. Every racer gives the edge
    // back inside its own claim, so a run that took one and did not put it
    // back leaves a file here that nothing else would notice.
    stored_records::assert_consumed(&sandbox);
}
