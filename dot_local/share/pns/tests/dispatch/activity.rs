use super::*;

#[test]
fn every_event_is_recorded_in_the_activity_ring_delivered_or_not() {
    // THE FILE THE JOURNAL CANNOT BE. The journal holds what the operator could
    // NOT have perceived; the recap's window is the opposite question, cards
    // that WERE delivered, glanced at and forgotten, so a delivered event has
    // to leave a line here while leaving the journal empty.
    let sandbox = Sandbox::new("activity-records-delivered");

    run(logged_event(&sandbox)
        .args(["--agent", "claude", "--state", "done"])
        .args(["--project", "dotfiles", "--detail", "a delivered summary"]));

    assert!(sandbox.fired("mobile"), "the away card really fired");
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
        .args(["--agent", "claude", "--state", "done"])
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
