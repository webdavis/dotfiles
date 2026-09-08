use super::*;

#[test]
fn an_event_raised_inside_a_focus_the_config_names_decorates_nothing_and_is_journaled() {
    // THE WHOLE FEATURE, end to end, and the only thing that proves the
    // composition root reads the store at all: a file in the sandbox's own
    // HOME, a mode name in the config, and an event that would otherwise
    // banner.
    //
    // SUPPRESSING IS STRICTLY MORE INFORMATIVE THAN NOT. macOS was going to
    // withhold this banner anyway, and pns posting it regardless would believe
    // it delivered, so the event would never be journaled: no banner AND no
    // recap entry. Held back here it becomes a miss the catch-up hands over
    // once the Focus ends.
    let sandbox = Sandbox::new("focus-named-mode");
    record_every_event(&sandbox);
    sandbox.write_focus_store(A_CUSTOM_FOCUS, ITS_NAME);
    sandbox.write_config(&focus_config("\"Casually Concerned\""));

    run(&mut focus_event(&sandbox));

    assert!(
        events(&sandbox, "macos-banner").is_empty(),
        "the Focus swallowed the banner"
    );
    assert!(
        events(&sandbox, "mobile").is_empty(),
        "and the card PNS_FORCE_PHONE asked for with it: a mute a producer can \
         override is not a mute"
    );
    let logged = events(&sandbox, "hermes");
    assert_eq!(
        logged.len(),
        1,
        "THE RECORD IS NEVER SUPPRESSED: the durable stream is what the \
         catch-up reads to say what was missed: {logged:?}"
    );
    let waiting = journal(&sandbox);
    assert_eq!(waiting.len(), 1, "exactly one miss was queued: {waiting:?}");
    assert_eq!(
        field(waiting.last().expect("a journal"), "detail"),
        "the live turn",
        "and it is this event's: {waiting:?}"
    );
    let ring = decisions(&sandbox);
    assert_eq!(ring.len(), 1, "one decision was recorded: {ring:?}");
    assert!(
        ring[0].contains("muted=no focus=yes"),
        "TWO FIELDS RATHER THAN ONE: `pns quiet` and a macOS Focus send the \
         operator to completely different places: {}",
        ring[0]
    );
    assert!(
        ring[0].contains("force_phone=yes"),
        "the force really was set, which is what makes the held card a verdict \
         rather than a surface that never offered one: {}",
        ring[0]
    );
    assert!(
        ring[0].contains("plan=banner:no,card:no,pulse:no"),
        "and the plan says all three were held, every one of which the sibling \
         test shows firing in this same world: {}",
        ring[0]
    );
}

#[test]
fn an_event_raised_inside_a_focus_the_config_never_named_is_delivered_as_usual() {
    // PER-MODE POLICY IS THE WHOLE POINT, and this is the half that makes the
    // feature usable. MEASURED on this operator's machine: a Focus was
    // asserted for 95% of one day, so a gate that fired on ANY active Focus
    // would be a mute with no expiry and nothing on screen to explain it.
    let sandbox = Sandbox::new("focus-unnamed-mode");
    record_every_event(&sandbox);
    sandbox.write_focus_store(A_CUSTOM_FOCUS, ITS_NAME);
    sandbox.write_config(&focus_config("\"Sleep\", \"Coding\""));

    run(&mut focus_event(&sandbox));

    assert_eq!(
        events(&sandbox, "macos-banner").len(),
        1,
        "a Focus nobody named silences nothing"
    );
    assert!(
        journal(&sandbox).is_empty(),
        "and a delivered event is not a miss"
    );
    let ring = decisions(&sandbox);
    assert!(
        ring[0].contains("muted=no focus=no"),
        "the log says the Focus decided nothing here: {}",
        ring[0]
    );
    // THE CONTROL FOR THE TEST ABOVE, and the reason both run in one world:
    // all three decorations really were on this plan, so the three `no`s next
    // door are a Focus holding them and not a surface that never offered them.
    assert_eq!(
        events(&sandbox, "mobile").len(),
        1,
        "the forced card fired here"
    );
    assert!(
        ring[0].contains("plan=banner:yes,card:yes,pulse:yes"),
        "and the plan carried all three: {}",
        ring[0]
    );
}

#[test]
fn a_focus_store_that_cannot_be_read_costs_no_notification_at_all() {
    // THE FAIL DIRECTION, and the one a reviewer should attack first. This is
    // a private, undocumented Apple store: it can be gated behind Full Disk
    // Access, moved, or given a new schema by any macOS update. Failing closed
    // would silence every banner, card and pulse from that morning on, with
    // nothing on screen to say why; failing open costs one interruption the
    // operator asked not to have, and `pns doctor` reports the unreadable
    // store on demand.
    for (label, plant) in [
        ("no store at all", false),
        ("something at the path that is not a file", true),
    ] {
        let sandbox = Sandbox::new(&format!("focus-unreadable-{}", plant as u8));
        record_every_event(&sandbox);
        sandbox.write_config(&focus_config("\"Casually Concerned\""));
        if plant {
            std::fs::create_dir_all(sandbox.path("Library/DoNotDisturb/DB/Assertions.json"))
                .expect("a directory where the store should be");
        }

        run(&mut present_event(&sandbox));

        assert_eq!(events(&sandbox, "macos-banner").len(), 1, "case: {label}");
        assert!(
            journal(&sandbox).is_empty(),
            "and a delivered event is not a miss: {label}"
        );
    }
}
