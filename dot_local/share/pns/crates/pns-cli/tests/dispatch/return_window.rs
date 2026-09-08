use super::*;

#[test]
fn a_present_event_moves_the_last_present_marker_and_an_away_event_does_not() {
    // THE WINDOW'S NEAR EDGE. A continuously present operator moves it on every
    // event, so their window is seconds wide and never trips the threshold;
    // an away operator leaves it where it was, which is what makes the window
    // grow across an absence.
    let away = Sandbox::new("marker-away");
    run(logged_event(&away).args(["--agent", "claude", "--state", "done", "--detail", "x"]));
    assert!(away.fired("mobile"), "the away row really was taken");
    assert_eq!(
        last_present(&away),
        None,
        "an away event marked the operator present: {:?}",
        state_files(&away)
    );

    let present = Sandbox::new("marker-present");
    run(&mut present_event(&present));
    let marked = last_present(&present).expect("a present event leaves the marker");
    assert!(
        marked > 1_700_000_000,
        "the marker holds this run's own clock read: {marked}"
    );
}

#[test]
fn an_activity_window_with_no_marker_to_open_it_recaps_nothing_and_still_catches_up() {
    // A FRESH INSTALL MUST NOT RECAP ALL OF HISTORY. Without a marker there is
    // no near edge, so there is no window, and no window is no recap however
    // full the ring is.
    //
    // AND THE CATCH-UP STILL FIRES, which is the half that says WHICH no-recap
    // this is. Reading an absent marker as epoch zero recaps the whole ring;
    // reading it as "another event is holding the window" delivers nothing at
    // all. Both are wrong and only the queued card tells them apart, so the
    // journal is planted and its card is asserted.
    let sandbox = Sandbox::new("recap-no-marker");
    record_every_event(&sandbox);
    std::fs::write(
        activity_path(&sandbox),
        planted_activity(MIN_EVENTS * 3, 1800, Some(0)),
    )
    .expect("the ring");
    std::fs::write(journal_path(&sandbox), planted_journal(2)).expect("the journal");

    run(&mut present_event(&sandbox));

    let raised = events(&sandbox, "macos-banner");
    assert_eq!(
        raised.len(),
        2,
        "the live event and the catch-up card, with no recap riding along: {raised:?}"
    );
    assert_eq!(raised[0]["state"], "done", "{raised:?}");
    let body = raised[1]["detail"].as_str().expect("a detail");
    assert!(
        body.starts_with("2 missed notifications. "),
        "a ring nobody opened a window on was recapped: {body}"
    );
}

#[test]
fn a_marker_no_reader_can_parse_opens_no_window_rather_than_one_from_epoch_zero() {
    // AN UNPARSEABLE EDGE IS NO EDGE, never an edge at epoch zero. There IS a
    // marker here, so the claim takes one and reads it; what it reads is not a
    // count. Reading that as zero opens a window over all of history and
    // recaps the whole ring, which is the same failure an absent marker has
    // its own test for and a different code path reaches it.
    //
    // AND THE EDGE HEALS, because the claim puts this event's own clock back
    // in place of what it could not read: the next window is a real one.
    let sandbox = Sandbox::new("recap-marker-unparseable");
    record_every_event(&sandbox);
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    std::fs::write(sandbox.path("state/last-present"), "not-an-epoch\n").expect("the marker");
    std::fs::write(
        activity_path(&sandbox),
        planted_activity(MIN_EVENTS * 3, 1800, Some(0)),
    )
    .expect("the ring");
    std::fs::write(journal_path(&sandbox), planted_journal(2)).expect("the journal");

    run(&mut present_event(&sandbox));

    let raised = events(&sandbox, "macos-banner");
    assert_eq!(
        raised.len(),
        2,
        "the live event and the catch-up card, with no recap riding along: {raised:?}"
    );
    let body = raised[1]["detail"].as_str().expect("a detail");
    assert!(
        body.starts_with("2 missed notifications. "),
        "a marker nobody could read was counted as epoch zero: {body}"
    );
    assert!(
        last_present(&sandbox).is_some_and(|edge| edge > 1_700_000_000),
        "the unreadable edge was not healed: {:?}",
        last_present(&sandbox)
    );
}

#[test]
fn events_stamped_at_the_markers_own_second_belong_to_it_and_not_to_the_window_after() {
    // THE NEAR EDGE IS EXCLUSIVE, and this is what that buys. The event that
    // MOVED the marker sits at exactly its epoch, and so does everything that
    // fired in the same second; counting those inside the next window makes a
    // burst at the desk read as a loud window opening at the instant it closed.
    // MEASURED with the edge inclusive: eight events in one second earned a
    // recap of an absence that never happened, and a second recap of a window
    // one had just been posted for.
    let sandbox = Sandbox::new("recap-marker-second");
    record_every_event(&sandbox);
    plant_marker(&sandbox, 1800);
    // THE SAME AGE AS THE MARKER, which is the whole fixture: twelve events at
    // that instant are twelve events the operator was present for.
    std::fs::write(activity_path(&sandbox), planted_activity(12, 1800, Some(4))).expect("the ring");
    std::fs::write(journal_path(&sandbox), planted_journal(2)).expect("the journal");

    run(&mut present_event(&sandbox));

    let raised = events(&sandbox, "macos-banner");
    assert_eq!(raised.len(), 2, "the live event and one card: {raised:?}");
    let body = raised[1]["detail"].as_str().expect("a detail");
    assert!(
        body.starts_with("2 missed notifications. "),
        "the marker's own second was counted as the window after it: {body}"
    );
}

#[test]
fn a_window_under_the_threshold_delivers_the_catch_up_card_unchanged() {
    // THE SLICE-13 CARD, VERBATIM. A quiet window is not a recap: the operator
    // stepped away for two events, so what they get back is the queue they
    // missed and nothing else. THE LIVE EVENT COUNTS ITSELF, which is why the
    // ring is planted two under the threshold rather than one.
    let sandbox = Sandbox::new("recap-under-threshold");
    record_every_event(&sandbox);
    plant_marker(&sandbox, 3600);
    std::fs::write(
        activity_path(&sandbox),
        planted_activity(MIN_EVENTS - 2, 1800, Some(0)),
    )
    .expect("the ring");
    std::fs::write(journal_path(&sandbox), planted_journal(2)).expect("the journal");

    run(&mut present_event(&sandbox));

    let raised = events(&sandbox, "macos-banner");
    assert_eq!(
        raised.len(),
        2,
        "the live event and ONE catch-up card: {raised:?}"
    );
    assert_eq!(raised[1]["state"], "missed", "{raised:?}");
    let body = raised[1]["detail"].as_str().expect("a detail");
    assert!(
        body.starts_with("2 missed notifications. "),
        "the under-threshold card is slice 13's, unchanged: {body}"
    );
}

#[test]
fn a_window_over_the_threshold_delivers_one_recap_card_with_what_needs_you_first() {
    // THE ONE-CARD RULE. Two layers were locked, phone and Discord, and slice
    // 13 already cards at this same return moment; a recap that raised its own
    // would put two cards on the phone in one moment. So the catch-up site
    // composes at most ONE card and this is the loud shape of it.
    let sandbox = Sandbox::new("recap-over-threshold");
    record_every_event(&sandbox);
    plant_marker(&sandbox, 3600);
    std::fs::write(activity_path(&sandbox), planted_activity(12, 1800, Some(4))).expect("the ring");
    std::fs::write(journal_path(&sandbox), planted_journal(2)).expect("the journal");

    run(&mut present_event(&sandbox));

    let raised = events(&sandbox, "macos-banner");
    assert_eq!(
        raised.len(),
        2,
        "the live event and ONE recap card, never two cards: {raised:?}"
    );
    assert_eq!(raised[0]["state"], "done", "the live event goes first");
    assert_eq!(raised[1]["agent"], "pns", "{raised:?}");
    assert_eq!(raised[1]["state"], "missed", "{raised:?}");
    let body = raised[1]["detail"].as_str().expect("a detail");
    // BOTH ARE FOUND FIRST and then compared: an `Option` compare answers true
    // for an item that is not on the card at all.
    let urgent = body
        .find("blocked")
        .unwrap_or_else(|| panic!("the blocked item never reached the card: {body}"));
    let counts = body
        .find("13 events")
        .unwrap_or_else(|| panic!("the window's own count never reached the card: {body}"));
    assert!(
        urgent < counts,
        "the counts came before the urgent item: {body}"
    );
    // TWELVE PLANTED PLUS THIS EVENT'S OWN: the activity ring records every
    // event, and the live one is inside the window it opened.
    assert!(body.contains("13 events"), "{body}");
    assert!(body.contains("2 missed"), "{body}");
    assert!(body.ends_with("recap in #pns"), "{body}");
    assert!(
        journal(&sandbox).is_empty(),
        "the journal was consumed: {:?}",
        state_files(&sandbox)
    );
}
