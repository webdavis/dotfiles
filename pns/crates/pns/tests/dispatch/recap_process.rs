use super::*;

#[test]
fn the_digest_reaches_discord_from_a_process_the_event_never_waited_for() {
    // THE ONE TEST THAT PROVES THE ASYNC LEG EXISTS, and it only proves it
    // because the durable channel PARKS on the recap. The engine has never
    // spawned anything it did not wait for, and the return moment is reached
    // from `pns hook prompt`, which the harness does not background, so a
    // recap rendered in this process would sit in front of a human's prompt.
    // The assertion is the parent's own exit while the recap is still stuck in
    // a channel: a build that renders the recap in-process cannot reach it.
    let sandbox = Sandbox::new("recap-detached-child");
    record_every_event(&sandbox);
    hermes_parks_on_the_recap(&sandbox);
    loud_window(&sandbox);

    let mut command = present_event(&sandbox);
    let mut started = command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("the engine starts");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while started.try_wait().expect("the child is waitable").is_none() {
        if std::time::Instant::now() >= deadline {
            // RELEASED BEFORE THE PANIC, so the parked stub is not left
            // holding a sandbox this test is about to delete.
            let _ = started.kill();
            let _ = started.wait();
            std::fs::write(sandbox.path(RELEASE), "").expect("the release");
            panic!("the event was still waiting on the recap it spawned");
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let done = started.wait_with_output().expect("the child is waitable");
    assert!(done.status.success(), "the event failed: {}", stderr(&done));
    // AND NOTHING WAS POSTED BEFORE IT EXITED, which is what makes the exit
    // above evidence rather than a coincidence of timing.
    assert!(
        events(&sandbox, "hermes")
            .iter()
            .all(|event| event["state"] != "recap"),
        "the recap was already posted when the event exited: {:?}",
        events(&sandbox, "hermes")
    );
    std::fs::write(sandbox.path(RELEASE), "").expect("the release");

    let posted = poll_until(|| {
        events(&sandbox, "hermes")
            .into_iter()
            .find(|event| event["state"] == "recap")
    })
    .unwrap_or_else(|| {
        panic!(
            "no recap reached the durable route: {:?}",
            events(&sandbox, "hermes")
        )
    });
    let body = posted["detail"].as_str().expect("a detail");
    assert!(
        body.starts_with("While you were away, "),
        "the recap's own header leads, which is what titles a forum thread: {body}"
    );
    assert!(body.contains("· 13 events"), "{body}");
    assert!(body.contains("NEEDS YOU"), "{body}");
    assert!(
        body.contains("claude/blocked p4: planted 4"),
        "the urgent entry reached the timeline: {body}"
    );
    assert!(
        body.lines().count() <= 25,
        "the recap ran past its line budget: {} lines",
        body.lines().count()
    );
}

#[test]
fn the_recap_child_runs_in_a_process_group_of_its_own() {
    // DETACHED MEANS THE GROUP TOO, and that half used to be claimed by a doc
    // comment rather than done. The return moment is reached from
    // `pns hook prompt`; a harness timing that hook out kills the process
    // GROUP, and so does SIGINT at the shell prompt the notifier runs from. A
    // child left in the parent's group dies with it, AFTER the window's edge
    // has already moved on, so that window can never fire again and the card
    // in the operator's hand points at a recap nobody is writing.
    //
    // THE GROUP IS READ AT THE CHANNEL, which is the one place a test can see
    // it: the channel is a grandchild and inherits whatever group its own
    // parent had. THE KIND AND THE GROUP RIDE ONE LINE, because three
    // processes reach this channel at one moment (the live event, the card the
    // parent raises for it, and the recap) and two files they all append to
    // can interleave differently from each other.
    let sandbox = Sandbox::new("recap-process-group");
    record_every_event(&sandbox);
    sandbox.stub_channel(
        "hermes",
        &format!(
            "payload=$(cat)\ngroup=$(ps -o pgid= -p $$ | tr -d ' ')\ncase \"$payload\" in\n  \
             *'\"state\":\"recap\"'*) printf 'recap %s\\n' \"$group\" >>\"{root}/hermes.pgid\" ;;\n  \
             *) printf 'event %s\\n' \"$group\" >>\"{root}/hermes.pgid\" ;;\nesac\n\
             printf '%s\\n' \"$payload\" >>\"{root}/hermes.events\"",
            root = sandbox.display()
        ),
    );
    loud_window(&sandbox);

    run(&mut present_event(&sandbox));

    // POLLED ON THE EVENT, which the stub writes AFTER the group, so a recap
    // that has been recorded has already recorded the group it ran in.
    poll_until(|| {
        events(&sandbox, "hermes")
            .into_iter()
            .find(|event| event["state"] == "recap")
    })
    .unwrap_or_else(|| panic!("no recap reached the durable route"));

    let recorded = std::fs::read_to_string(sandbox.path("hermes.pgid")).expect("the groups");
    let group_of = |kind: &str| -> Vec<&str> {
        recorded
            .lines()
            .filter_map(|line| line.strip_prefix(kind))
            .collect()
    };
    let recap = group_of("recap ");
    assert_eq!(recap.len(), 1, "one recap and one group: {recorded}");
    assert!(
        !group_of("event ").is_empty(),
        "nothing recorded the group the event itself ran in: {recorded}"
    );
    assert!(
        !group_of("event ").contains(&recap[0]),
        "the recap child stayed in the group that would be killed with the hook: {recorded}"
    );
}

#[test]
fn a_switched_off_digest_posts_no_recap_and_leaves_the_catch_up_card_alone() {
    // EACH SWITCH GATES ONLY ITS OWN DELIVERY. With the recap off, a loud
    // window is still a window: the marker still moves, the journal is still
    // claimed, and what the operator gets is slice 13's card, unchanged.
    let sandbox = Sandbox::new("recap-digest-off");
    record_every_event(&sandbox);
    sandbox.write_config(&recap_switched_off());
    loud_window(&sandbox);

    run(&mut present_event(&sandbox));

    let raised = events(&sandbox, "macos-banner");
    assert_eq!(raised.len(), 2, "the live event and one card: {raised:?}");
    let body = raised[1]["detail"].as_str().expect("a detail");
    assert!(
        body.starts_with("2 missed notifications. "),
        "the card is slice 13's, not the recap's: {body}"
    );
    // NO CHILD WAS EVER STARTED, and the card is the witness: the recap card is
    // the only thing that says "recap in #pns", and only a real spawn earns it.
    assert!(!body.contains("recap in #pns"), "{body}");
    assert!(
        events(&sandbox, "hermes")
            .iter()
            .all(|event| event["state"] != "recap"),
        "a recap was posted with the digest switched off: {:?}",
        events(&sandbox, "hermes")
    );
    assert!(
        last_present(&sandbox).is_some(),
        "the marker stopped moving because a switch was off"
    );
}
