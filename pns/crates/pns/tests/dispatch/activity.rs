use super::*;

#[test]
fn every_event_is_recorded_in_the_activity_ring_delivered_or_not() {
    // THE FILE THE JOURNAL CANNOT BE. The journal holds what the operator could
    // NOT have perceived; the recap's window is the opposite question, cards
    // that WERE delivered, glanced at and forgotten, so a delivered event has
    // to leave a line here while leaving the journal empty.
    let sandbox = Sandbox::new("activity-records-delivered");

    run(acknowledged_banner(&sandbox)
        .args(["--producer", "claude", "--state", "done"])
        .args(["--project", "dotfiles", "--detail", "a delivered summary"]));

    assert!(
        sandbox.path("notifier.args").exists(),
        "the native banner ran"
    );
    assert!(
        journal(&sandbox).is_empty(),
        "so nothing was missed and the journal stayed empty"
    );
    let recorded = activity(&sandbox);
    assert_eq!(recorded.len(), 1, "exactly one entry: {recorded:?}");
    for (name, expected) in [
        ("agent", "claude"),
        ("state", "done"),
        ("project", "dotfiles"),
        ("detail", "a delivered summary"),
    ] {
        assert_eq!(field(&recorded[0], name), expected, "{name}: {recorded:?}");
    }
}

#[test]
fn a_full_activity_ring_prunes_to_its_own_depth_instead_of_collapsing_to_one_line() {
    // TWO FAILURES IN ONE FIXTURE, and the ring is planted at its WORST CASE
    // because only that shows the second one. The DEPTH is the ordinary half:
    // a ring already at its cap loses its oldest entry to this event. The READ
    // CEILING is the silent half: a full ring of control bytes is over 400 KiB,
    // so a reader capped at the decision ring's 256 KiB refuses it, the
    // append's own heal fires, and the file collapses to the ONE line it just
    // wrote, exactly when it is fullest and with nothing said.
    let sandbox = Sandbox::new("activity-prune");
    std::fs::write(
        activity_path(&sandbox),
        escape_heavy_activity(ACTIVITY_KEPT),
    )
    .expect("the ring");
    let planted = std::fs::metadata(activity_path(&sandbox))
        .expect("the ring")
        .len();
    assert!(
        planted > SHARED_READ_MAX,
        "the fixture has to be past the SHARED read cap to say anything: {planted} bytes"
    );

    run(logged_event(&sandbox)
        .args(["send", "--producer", "claude", "--state", "done"])
        .args(["--detail", "the newest event"]));

    let recorded = activity(&sandbox);
    // THE APPEND REALLY RAN, asserted before the count: a ring nothing wrote is
    // still exactly its planted depth, so the count alone would pass a build
    // that never records anything at all.
    assert_eq!(
        field(recorded.last().expect("a ring"), "detail"),
        "the newest event",
        "the newest entry is this event's: {} lines",
        recorded.len()
    );
    assert_eq!(
        recorded.len(),
        ACTIVITY_KEPT,
        "the ring kept its own depth rather than collapsing or growing"
    );
    assert_eq!(
        field(&recorded[0], "project"),
        "planted 1",
        "the oldest was the one dropped"
    );
}

#[test]
fn two_activity_events_racing_a_full_ring_lose_neither_line() {
    // PORTED from the deleted policy-audit ring's own
    // `two_policy_settings_changes_racing_the_prune_lose_neither_line`, the only
    // test that raced two processes on a ring already at its cap, where the
    // prune runs. `Ring::Activity` still goes through the shared
    // `rows::append` production path Presence uses too, so this keeps that
    // race pinned. EVERY COMMAND IS BUILT BEFORE THE FIRST SPAWN, and both are
    // spawned before either is waited on, which is what makes them contend.
    let sandbox = Sandbox::new("activity-prune-race");
    std::fs::write(
        activity_path(&sandbox),
        escape_heavy_activity(ACTIVITY_KEPT),
    )
    .expect("the ring");

    let mut racer_one = logged_event(&sandbox);
    racer_one
        .args(["send", "--producer", "racer-one", "--state", "done"])
        .args(["--detail", "the first racer"]);
    let mut racer_two = logged_event(&sandbox);
    racer_two
        .args(["send", "--producer", "racer-two", "--state", "done"])
        .args(["--detail", "the second racer"]);
    let child_one = racer_one.spawn().expect("the first racer starts");
    let child_two = racer_two.spawn().expect("the second racer starts");
    for mut child in [child_one, child_two] {
        assert!(
            child.wait().expect("the racer is waitable").success(),
            "a racer failed"
        );
    }

    let recorded = activity(&sandbox);
    assert_eq!(
        recorded.len(),
        ACTIVITY_KEPT,
        "the ring keeps its own bound under a race exactly as it does one at a time: {} lines",
        recorded.len()
    );
    assert!(
        recorded
            .iter()
            .any(|line| field(line, "agent") == "racer-one"),
        "the first racer survives the concurrent prune: {recorded:?}"
    );
    assert!(
        recorded
            .iter()
            .any(|line| field(line, "agent") == "racer-two"),
        "the second racer survives the concurrent prune: {recorded:?}"
    );
    assert!(
        !recorded
            .iter()
            .any(|line| field(line, "project") == "planted 0"),
        "the oldest planted entry is dropped, exactly as it is outside a race: {recorded:?}"
    );
    assert!(
        !recorded
            .iter()
            .any(|line| field(line, "project") == "planted 1"),
        "two new events push the kept window forward by two, so the second-oldest \
         planted entry is also dropped rather than resurrected by a stale publish: {recorded:?}"
    );
}
