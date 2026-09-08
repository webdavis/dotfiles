use super::*;

#[test]
fn a_present_event_delivers_one_extra_notification_carrying_the_whole_journal() {
    // THE RETURN TRANSITION IS THIS EVENT. The operator is at the desk with
    // the origin pane out of sight, so the live turn earns a banner, and the
    // queue rides out on the same legs that banner did.
    let sandbox = Sandbox::new("replay-delivers");
    record_every_event(&sandbox);
    std::fs::write(journal_path(&sandbox), planted_journal(2)).expect("the journal");

    let output = run(&mut present_event(&sandbox));

    // NOTHING IS PRINTED. The event path prints only what a REPORTING leg
    // said, and this rides an event whose stdout a harness hook reads.
    assert_eq!(
        stdout(&output),
        "",
        "the replay reached the hook's own stdout"
    );
    assert_eq!(
        stderr(&output),
        "",
        "the replay reached the hook's own stderr"
    );
    let raised = events(&sandbox, "macos-banner");
    assert_eq!(
        raised.len(),
        2,
        "the live event and ONE replay carrying both entries: {raised:?}"
    );
    assert_eq!(raised[0]["state"], "done", "the live event goes first");
    // ONE SYNTHETIC EVENT, visibly not a live agent card: a replayed card that
    // looked live would be lying about time.
    assert_eq!(raised[1]["agent"], "pns", "{raised:?}");
    assert_eq!(raised[1]["state"], "missed", "{raised:?}");
    assert_eq!(raised[1]["title"], "pns \u{b7} missed", "{raised:?}");
    // EMPTY PROJECT AND BRANCH, because a batch spans both; empty pane,
    // because an id from an hour ago may name a pane that no longer exists.
    for empty in ["project", "branch", "pane"] {
        assert_eq!(raised[1][empty], "", "{empty}: {raised:?}");
    }
    let body = raised[1]["detail"].as_str().expect("a detail");
    assert!(
        body.starts_with("2 missed notifications. "),
        "the true count leads: {body}"
    );
    // BOTH ARE FOUND FIRST, and then compared. An `Option` compare answers
    // true for an entry that is not in the body at all, so the comparison
    // alone would pass a card carrying neither.
    let newest = body
        .find("planted 1")
        .expect("the newest entry is in the body");
    let oldest = body
        .find("planted 0")
        .expect("the oldest entry is in the body");
    assert!(
        newest < oldest,
        "newest first, because the preview cuts from the start: {body}"
    );
    // THE LEGS ARE THIS DECISION'S OWN, VERBATIM: the durable log is one of
    // them, so it is handed the same synthetic event the banner was.
    let logged = events(&sandbox, "hermes");
    assert_eq!(
        logged.len(),
        2,
        "the replay rode only the banner leg: {logged:?}"
    );
    assert_eq!(logged[1]["state"], "missed", "{logged:?}");
    assert_eq!(
        logged[1]["detail"], raised[1]["detail"],
        "the two legs were handed different bodies"
    );
    stored_records::assert_consumed(&sandbox);
}

#[test]
fn a_replay_is_never_a_second_event_in_the_ring_or_the_journal() {
    // THE LOOP THIS CLOSES. Fed back through the one event path, the replay
    // would take a second decision, write a second ring line for something
    // that is not an event, fire a second pulse and RE-JOURNAL itself, so the
    // next replay would replay the replay. One ring line, naming the live
    // event, is what says the replay stayed a dispatch.
    let sandbox = Sandbox::new("replay-not-an-event");
    record_every_event(&sandbox);
    std::fs::write(journal_path(&sandbox), planted_journal(2)).expect("the journal");

    run(&mut present_event(&sandbox));

    assert_eq!(
        events(&sandbox, "macos-banner").len(),
        2,
        "the replay really was delivered, which is what makes the counts below mean anything"
    );
    let recorded = decisions(&sandbox);
    assert_eq!(recorded.len(), 1, "one event, one line: {recorded:?}");
    assert!(
        recorded[0].contains(" claude/done "),
        "and it is the LIVE event's: {recorded:?}"
    );
    assert!(
        !recorded[0].contains("pns/missed"),
        "the replay recorded a decision of its own: {recorded:?}"
    );
    stored_records::assert_consumed(&sandbox);
}

#[test]
fn an_away_event_delivers_no_replay_and_leaves_the_journal_byte_identical() {
    // AWAY IS WHERE MISSES ARE MADE AND NEVER WHERE THEY ARE DELIVERED. The
    // Away row always cards, so without the surface clause this row would
    // flush the queue at the phone of an operator who has not come back.
    let sandbox = Sandbox::new("replay-away");
    record_every_event(&sandbox);
    std::fs::write(journal_path(&sandbox), planted_journal(2)).expect("the journal");
    let before = std::fs::read(journal_path(&sandbox)).expect("the journal");

    run(logged_event(&sandbox).args(["--agent", "claude", "--state", "done", "--detail", "x"]));

    let carded = events(&sandbox, "mobile");
    assert_eq!(
        carded.len(),
        1,
        "the away card fired and nothing rode along: {carded:?}"
    );
    assert_eq!(carded[0]["state"], "done", "{carded:?}");
    assert_eq!(
        std::fs::read(journal_path(&sandbox)).expect("the journal"),
        before,
        "the journal was touched"
    );
    let retained = stored_records::text(&sandbox, "journal");
    assert!(
        retained.as_bytes().starts_with(&before),
        "the planted queue changed"
    );
    let waiting = journal(&sandbox);
    assert_eq!(waiting.len(), 3, "the unconfirmed live send adds one miss");
    assert_eq!(field(waiting.last().unwrap(), "detail"), "x");
    assert!(
        stored_records::claims(&sandbox).is_empty(),
        "the queue was claimed"
    );
}

#[test]
fn a_switched_off_replay_card_delivers_no_catch_up_and_leaves_the_journal_whole() {
    // THE SWITCH GOES IN FRONT OF THE CLAIM, never after it: claiming renames
    // the journal out of the way, so a return after that point would consume
    // the queue and deliver nothing, which is worse than either half. The
    // byte-identical journal is what says the switch is in front, and the
    // single banner is what says it fired at all.
    let sandbox = Sandbox::new("replay-card-off");
    record_every_event(&sandbox);
    sandbox.write_config(&card_switched_off());
    std::fs::write(journal_path(&sandbox), planted_journal(2)).expect("the journal");
    let before = std::fs::read(journal_path(&sandbox)).expect("the journal");

    let output = run(&mut present_event(&sandbox));

    let raised = events(&sandbox, "macos-banner");
    assert_eq!(
        raised.len(),
        1,
        "the live event alone, with no catch-up riding along: {raised:?}"
    );
    assert_eq!(raised[0]["state"], "done", "{raised:?}");
    // READ AS AN OPTION, because the failure this pins is the journal being
    // GONE: an `expect` here would report the operating system's words for a
    // missing file instead of what happened, which is a queue consumed by a
    // card nobody was sent.
    assert_eq!(
        std::fs::read(journal_path(&sandbox)).ok(),
        Some(before.clone()),
        "the queue was consumed by a card nobody was sent"
    );
    // A SETTING IS NOT A COMPLAINT: an operator who switched the card off is
    // not told about it on every event.
    assert_eq!(stderr(&output), "", "the switch printed something");
    assert_eq!(stored_records::text(&sandbox, "journal").as_bytes(), before);
    assert!(
        stored_records::claims(&sandbox).is_empty(),
        "the queue was claimed"
    );
}

#[test]
fn a_switched_off_replay_card_still_journals_the_misses_it_makes() {
    // THE JOURNAL ALWAYS RECORDS, and that is structural rather than
    // remembered: `record_missed` never learns the switch exists. Putting the
    // switch there instead would empty the queue behind the card, so
    // switching the card back on would have nothing to deliver.
    //
    // GUARD. It was already green before the switch existed and it stays
    // green for as long as the switch stays out of the write site, so its
    // teeth are the mutation that moves the gate INTO `record_missed`: that
    // is the change this is here to turn red.
    let sandbox = Sandbox::new("replay-card-off-journals");
    record_every_event(&sandbox);
    sandbox.write_config(&card_switched_off());

    // An acknowledged native banner leaves no miss even with replay disabled.
    run(acknowledged_banner(&sandbox)
        .args(["--agent", "claude", "--state", "done", "--detail", "seen"]));
    assert!(
        journal(&sandbox).is_empty(),
        "a delivered event journaled itself: {:?}",
        journal(&sandbox)
    );

    // A MUTE ZEROES THE PLAN, which is a miss by every reading.
    mute(&sandbox);
    run(logged_event(&sandbox).args(["--agent", "claude", "--state", "done", "--detail", "muted"]));

    let waiting = journal(&sandbox);
    assert_eq!(waiting.len(), 1, "the miss was recorded: {waiting:?}");
    assert_eq!(
        field(&waiting[0], "detail"),
        "muted",
        "and it is the missed event's: {waiting:?}"
    );
}

#[test]
fn a_muted_event_queues_its_own_miss_and_replays_nothing() {
    // THE MUTE IS FREE, and it is exactly what the operator asked for: a mute
    // zeroes the plan, so a muted run has no decoration and cannot flush the
    // queue it is filling. Nothing reads `muted` on the replay path.
    let sandbox = Sandbox::new("replay-muted");
    record_every_event(&sandbox);
    mute(&sandbox);
    std::fs::write(journal_path(&sandbox), planted_journal(2)).expect("the journal");

    run(present_event(&sandbox).env_remove("PNS_SKIP_PHONE"));

    let waiting = journal(&sandbox);
    assert_eq!(
        waiting.len(),
        3,
        "its own miss joined the queue: {waiting:?}"
    );
    assert_eq!(
        field(waiting.last().expect("a journal"), "detail"),
        "the live turn",
        "and it is this event's: {waiting:?}"
    );
    assert!(
        events(&sandbox, "macos-banner").is_empty(),
        "the mute swallowed the banner, so nothing carried a replay"
    );
    let logged = events(&sandbox, "hermes");
    assert_eq!(
        logged.len(),
        1,
        "the durable log is exempt from the mute and still saw ONE event: {logged:?}"
    );
    assert_eq!(logged[0]["state"], "done", "{logged:?}");
}
