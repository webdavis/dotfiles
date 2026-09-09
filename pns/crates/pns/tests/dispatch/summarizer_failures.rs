use super::*;

#[test]
fn a_summarizer_that_exits_non_zero_falls_to_the_plain_list_and_says_so() {
    // A FAILED RUN IS NOT AN EMPTY NIGHT. The command answered, and what it
    // answered was that it could not do this; posting its silence as though the
    // window were quiet is the one reading that loses information.
    assert_fell_back_to_the_plain_list(&recap_summarized_badly(
        "recap-summarizer-exits-one",
        "",
        "exit 1",
    ));
}

#[test]
fn a_summarizer_that_answers_with_nothing_falls_to_the_plain_list_and_says_so() {
    // EXIT ZERO AND NOT ONE WORD, which a backend does when it refuses a prompt
    // or when its model is missing. Success is not an answer.
    assert_fell_back_to_the_plain_list(&recap_summarized_badly(
        "recap-summarizer-says-nothing",
        "",
        "exit 0",
    ));
}

#[test]
fn a_summarizer_past_a_short_deadline_returns_no_partial_answer() {
    use pns_application::Summarizer;
    let summarizer = pns_adapters::ProcessSummarizer;
    // A complete answer distinguishes a deadline refusal from always returning
    // None. The unchanged CLI cases below own the plain-list notice and body.
    assert_eq!(
        summarizer.summarize(
            &["/bin/echo".into(), "a complete answer".into()],
            std::time::Duration::from_millis(300),
            "the window",
        ),
        Some(vec!["a complete answer".into()]),
    );
    let started = std::time::Instant::now();
    assert_eq!(
        summarizer.summarize(
            &[
                "/bin/sh".into(),
                "-c".into(),
                "printf 'a partial answer\\n'; exec /bin/sleep 30".into(),
            ],
            std::time::Duration::from_millis(40),
            "the window",
        ),
        None,
        "the owned process exceeded its deadline; partial text is not an answer",
    );
    assert!(started.elapsed() < std::time::Duration::from_millis(500));
}

#[test]
fn a_summarizer_that_never_answers_costs_the_card_nothing() {
    // THE MODEL IS NEVER ON THE EVENT PATH, and this is where that is proved
    // rather than promised. The stub will not return until this test says so,
    // which the parked-channel test's own comment explains: a summarizer run in
    // the parent could satisfy a poll-afterwards assertion just as well, and it
    // cannot satisfy this one, because what is asserted is the parent's own
    // exit while the summarizer is still stuck.
    let sandbox = Sandbox::new("recap-summarizer-parks");
    record_every_event(&sandbox);
    sandbox.write_config(&recap_summarized_by(""));
    loud_window(&sandbox);

    let mut command = present_event(&sandbox);
    stub_summarizer(
        &sandbox,
        &mut command,
        &format!(
            // BOUNDED ANYWAY, at ten seconds, so a broken build fails rather
            // than hangs.
            "for _ in $(seq 1 200); do [ -e \"{root}/{RELEASE}\" ] && break; sleep 0.05; done",
            root = sandbox.display()
        ),
    );
    let mut started = command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("the engine starts");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while started.try_wait().expect("the child is waitable").is_none() {
        if std::time::Instant::now() >= deadline {
            // RELEASED BEFORE THE PANIC, so the parked stub is not left holding
            // a sandbox this test is about to delete.
            let _ = started.kill();
            let _ = started.wait();
            std::fs::write(sandbox.path(RELEASE), "").expect("the release");
            panic!("the event was waiting on a summarizer it should never have run");
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let done = started.wait_with_output().expect("the child is waitable");
    assert!(done.status.success(), "the event failed: {}", stderr(&done));

    // AND THE CARD IS ALREADY IN THE OPERATOR'S HAND while the model is still
    // thinking, which is the whole two-layer arrangement: the phone layer is
    // composed from the entries and owes the summarizer nothing.
    let raised = events(&sandbox, "macos-banner");
    assert_eq!(
        raised.len(),
        2,
        "the live event and one recap card: {raised:?}"
    );
    assert_eq!(
        raised[1]["detail"], "claude · blocked · p4. 13 events, 2 missed. recap in #pns",
        "the card waited for the model or was composed by it: {raised:?}"
    );
    assert!(
        events(&sandbox, "hermes")
            .iter()
            .all(|event| event["state"] != "recap"),
        "the recap was posted while its summarizer was still parked: {:?}",
        events(&sandbox, "hermes")
    );

    std::fs::write(sandbox.path(RELEASE), "").expect("the release");
    // AND THE DIGEST STILL ARRIVES, late and plain, which is the outcome the
    // whole ladder is arranged to end at.
    assert_fell_back_to_the_plain_list(&posted_recap(&sandbox));
}

#[test]
fn a_summarizer_that_is_not_installed_at_all_falls_to_the_plain_list_and_says_so() {
    // THE FIRST RUNG AN OPERATOR MEETS, on any machine where the backend named
    // in the table is not installed yet. It is the one rung that never reaches
    // `answer` at all: the spawn itself fails, and what has to happen is the
    // same thing that happens for every other way of not answering.
    let sandbox = Sandbox::new("recap-summarizer-not-installed");
    record_every_event(&sandbox);
    sandbox.write_config(&format!(
        "{EVERY_DISPATCHED_CHANNEL}[recap]\nsummarizer = [\"pns-no-such-summarizer\"]\n"
    ));
    loud_window(&sandbox);

    run(&mut present_event(&sandbox));

    assert_fell_back_to_the_plain_list(&posted_recap(&sandbox));
}

#[test]
fn a_summarizer_answering_in_bytes_that_are_not_text_falls_to_the_plain_list() {
    // THE SEAM READS LOSSILY, so invalid bytes reach the composition as
    // replacement characters rather than as an error. A backend mid-crash, or
    // one writing a binary it thought was a string, would otherwise put those
    // glyphs in the operator's timeline; the same seam's idle-counter reader
    // treats one replacement character as proof the whole reading is corrupt,
    // and a timeline is not more trustworthy than an idle counter.
    assert_fell_back_to_the_plain_list(&recap_summarized_badly(
        "recap-summarizer-invalid-bytes",
        "",
        "printf 'the night went \\377\\376 well\\n'",
    ));
}

#[test]
fn an_empty_window_says_so_itself_and_never_starts_a_summarizer_at_all() {
    // A WINDOW WITH NOTHING IN IT HAS NO NIGHT TO SUMMARIZE, and a model handed
    // "- nothing was recorded in this window" under an instruction to rewrite it
    // as a timeline will happily write one. THE HAND-RUN RECAP IS EXACTLY WHERE
    // THAT LANDS: the event path never posts over an empty window, and
    // `pns recap --since ... --until ...` is the drill an operator runs at a
    // quiet stretch to check a route, which is also where an invented line is
    // most likely to be believed.
    let sandbox = Sandbox::new("recap-summarizer-empty-window");
    record_every_event(&sandbox);
    sandbox.write_config(&recap_summarized_by(""));

    let mut command = logged_event(&sandbox);
    command.args(["recap", "--since", "1756500000", "--until", "1756500600"]);
    stub_summarizer(
        &sandbox,
        &mut command,
        &format!(
            "printf 'ran\\n' >>'{}'\nprintf '%s\\n' '23:04 the branch landed cleanly'",
            sandbox.path("summarizer.ran").display()
        ),
    );
    run(&mut command);

    let body = posted_recap(&sandbox);
    assert!(
        body.contains("- nothing was recorded in this window"),
        "the empty window did not say so itself: {body}"
    );
    assert!(
        !body.contains("the branch landed cleanly"),
        "a model wrote a night that never happened: {body}"
    );
    assert!(
        !sandbox.path("summarizer.ran").exists(),
        "a model was started to summarize nothing: {body}"
    );
}

#[test]
fn a_summarizer_answering_with_a_megabyte_gets_the_plain_list_posted_instead() {
    // THE SEAM READS UNTIL THE CHILD STOPS TALKING, bounded in time and not in
    // bytes, so a backend that streams for its whole deadline really does hand
    // back everything it wrote. A megabyte of it is not a timeline whatever it
    // says, and the plain list is the better message.
    assert_fell_back_to_the_plain_list(&recap_summarized_badly(
        "recap-summarizer-megabyte",
        "",
        "head -c 1000000 </dev/zero | tr '\\0' 'x'",
    ));
}
