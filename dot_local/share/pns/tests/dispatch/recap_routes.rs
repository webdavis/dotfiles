use super::*;

#[test]
fn a_machine_with_no_durable_route_never_points_a_card_at_a_recap_nothing_can_carry() {
    // "recap in #pns" IS A PROMISE, and a spawn alone cannot back it. A
    // started child still posts nothing when there is no durable channel: the
    // hermes leg answers Failed before it touches the network and the child
    // exits 0, so the phone said "recap in #pns" and #pns stayed empty.
    //
    // ASKED OF THE SELECTION, which is the one reading dispatch takes too, so
    // the promise on the card and the channel behind it cannot disagree. TWO
    // MACHINES HAVE NOWHERE FOR A RECAP TO GO and both are honest: one whose
    // config NAMES a roster without hermes, which is this test, and one with
    // no usable config at all, since hermes needs a route signed for before
    // it can carry anything and so is not in the core.
    let sandbox = Sandbox::new("recap-no-durable-route");
    record_every_event(&sandbox);
    sandbox
        .write_config("[plugins.mobile]\nenabled = true\ntype = \"moshi\"\n[plugins.macos-banner]\nenabled = true\n");
    loud_window(&sandbox);

    run(&mut present_event(&sandbox));

    let raised = events(&sandbox, "macos-banner");
    assert_eq!(raised.len(), 2, "the live event and one card: {raised:?}");
    let body = raised[1]["detail"].as_str().expect("a detail");
    assert!(
        !body.contains("recap in #pns"),
        "the card pointed at a recap no channel could carry: {body}"
    );
    assert!(
        body.starts_with("2 missed notifications. "),
        "and what it delivered instead is slice 13's card, unchanged: {body}"
    );
    assert!(
        !sandbox.fired("hermes"),
        "a recap reached a route the config turned off: {:?}",
        events(&sandbox, "hermes")
    );
}

#[test]
fn the_marker_advances_so_a_second_present_event_recaps_nothing() {
    // IDEMPOTENCE, as locked. Without the advance the second event counts the
    // same loud window and posts the same recap again, which is the exact
    // failure the marker exists to prevent. The two events run back to back
    // over ONE window, and exactly one recap may come out of it.
    let sandbox = Sandbox::new("recap-idempotent");
    record_every_event(&sandbox);
    loud_window(&sandbox);

    run(&mut present_event(&sandbox));
    run(&mut present_event(&sandbox));

    let raised = events(&sandbox, "macos-banner");
    assert_eq!(
        raised.len(),
        3,
        "two live events and ONE card between them: {raised:?}"
    );
    assert_eq!(
        raised
            .iter()
            .filter(|event| event["state"] == "missed")
            .count(),
        1,
        "the second event carded the same window again: {raised:?}"
    );
    // AND THE DISCORD HALF IS COUNTED THE SAME WAY, polled so a second child
    // that was slower than the first still fails this.
    poll_until(|| {
        events(&sandbox, "hermes")
            .into_iter()
            .find(|event| event["state"] == "recap")
    })
    .unwrap_or_else(|| panic!("the first event posted no recap at all"));
    assert_eq!(
        events(&sandbox, "hermes")
            .iter()
            .filter(|event| event["state"] == "recap")
            .count(),
        1,
        "the same window was recapped twice: {:?}",
        events(&sandbox, "hermes")
    );
}

#[test]
fn a_recap_told_a_window_it_cannot_read_prints_usage_exits_two_and_posts_nothing() {
    // A MODE, NOT A HOOK, so a typo is a refusal rather than a silent exit 0:
    // this is hand-runnable, and a recap the operator believes was posted is
    // worse than one that said it could not be. `event_mode` is what it used to
    // fall through to, which would have sent a notification about nothing.
    let sandbox = Sandbox::new("recap-usage");
    let output = logged_event(&sandbox)
        .args(["recap", "--since", "yesterday", "--until", "1756500000"])
        .output()
        .expect("the engine runs");

    assert_eq!(output.status.code(), Some(2), "stderr: {}", stderr(&output));
    assert!(
        stderr(&output).contains("pns recap --since <epoch> --until <epoch>"),
        "the usage names both bounds: {}",
        stderr(&output)
    );
    for channel in ["hermes", "mobile", "macos-banner"] {
        assert!(
            !sandbox.fired(channel),
            "{channel} was handed a recap over a window nobody could read"
        );
    }
}
