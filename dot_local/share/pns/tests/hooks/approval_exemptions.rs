use super::*;

#[test]
fn an_approval_is_forwarded_even_when_the_mobile_channel_is_switched_off() {
    // The forward is independent of plugin selection AND of the push token,
    // and neither is a coincidence worth leaving unpinned: a submission built
    // on the mobile channel's own config would couple the two silently, and an
    // operator who never set a token would lose approvals while every test
    // stayed green.
    //
    // MECHANISM-BOUND: the submission is read off the record, so this goes
    // RED at the endpoint switch for item 25 to rewrite.
    let sandbox = Sandbox::new("hook-blocked-channel-off");
    sandbox.write_config("[plugins.mobile]\nenabled = false\n[plugins.hermes]\nenabled = true\n");
    let mut command = sandbox.pns();
    command.env("PNS_IDLE_SECS", "99999");
    sandbox.stub_moshi(&mut command, 42);
    let output = hook_with(command, &sandbox, "blocked", r#"{"message":"may I"}"#);
    assert_eq!(output.status.code(), Some(42), "the operator's own answer");
    assert!(
        sandbox.path("moshi.argv").exists(),
        "a channel the operator turned off is not an approval they declined"
    );
    // THE ABSENT CARD IS NOT EVIDENCE THE CONFIG WAS READ. A forward that
    // happened suppresses pns's own phone leg whatever the config says, and
    // the same absence shows up with the channel enabled and with no config
    // file at all (measured, three ways). This line pins that no second card
    // appeared, and nothing about what silenced it; the exit code and the
    // submission above are what carry the selection exemption.
    assert!(!sandbox.fired("mobile"));
    assert!(sandbox.fired("hermes"), "the paper trail is still written");
}

#[test]
fn an_approval_is_forwarded_even_with_the_pane_in_plain_sight() {
    // VISIBILITY GATES NOTIFICATIONS AND NEVER APPROVALS, and today that is
    // STRUCTURAL rather than stated: `operator_surface`'s trait bound carries
    // no session-view probe at all, so the forward cannot consult one. A
    // refactor that routed the forward through the delivery decision would
    // break it without editing a line anyone reviewed, which is what this
    // catches. The mute twin exists for the same reason and this is its
    // sibling.
    //
    // WHAT THIS DOES NOT PROVE: `stub_herdr` records nothing, so nothing here
    // distinguishes "the session view answered Visible" from "it was never
    // consulted at all". The visibility READ is exercised by some twenty
    // tests in the dispatch suite; what this pins is the forward's
    // INDIFFERENCE to it, which is the half those tests cannot see.
    //
    // MECHANISM-BOUND: the submission is read off the record, so this goes
    // RED at the endpoint switch for item 25 to rewrite.
    let sandbox = Sandbox::new("hook-blocked-pane-visible");
    let mut command = sandbox.pns();
    command
        .env("PNS_IDLE_SECS", "99999")
        .env("HERDR_PANE_ID", "t1:p1");
    sandbox.stub_herdr(&mut command, true);
    sandbox.stub_moshi(&mut command, 42);
    let output = hook_with(command, &sandbox, "blocked", r#"{"message":"may I"}"#);
    assert_eq!(output.status.code(), Some(42), "the operator's own answer");
    assert!(
        sandbox.path("moshi.argv").exists(),
        "a pane in plain sight is not an answer to the prompt on it"
    );
}

#[test]
fn the_forward_reads_the_surface_and_never_the_card_overrides() {
    // TWO OVERRIDES THAT LOOK LIKE THE SAME QUESTION AND ARE NOT. Both are
    // applied to the delivery plan's `phone_card` and NEITHER is read by
    // `forward_to_moshi`, which compares the surface and nothing else. The
    // distinction is the one a second approval surface is most likely to
    // collapse, and each direction fails differently:
    //
    //   FORCE buys a PUSH, not a ROUND TRIP. At the desk it cards the phone
    //   and submits nothing, so there is no card to answer. Wiring the
    //   override into the forward would open a round trip nobody asked for,
    //   on the one surface where the prompt is already on screen.
    //
    //   SKIP suppresses PNS'S CARD, not the SUBMISSION. It is set by the
    //   blocked path itself once a submission succeeds, so reading it in the
    //   forward would be the forward gating on its own output; away, an
    //   operator with the variable set would lose approvals entirely.
    //
    // Both mutations were measured to pass the rest of the suite.
    let forced = Sandbox::new("hook-blocked-force-phone");
    let mut command = approval(&forced, 42);
    command
        .env("PNS_IDLE_SECS", "0")
        .env("PNS_FORCE_PHONE", "1");
    let output = hook_with(command, &forced, "blocked", CLAUDE_APPROVAL);
    assert_eq!(output.status.code(), Some(0), "no round trip, no decision");
    assert!(
        submissions(&forced).is_empty(),
        "the override buys a card, and a card is not a question anyone can answer"
    );
    assert!(
        forced.fired("mobile"),
        "the push it does buy still has to arrive"
    );

    let skipped = Sandbox::new("hook-blocked-skip-phone");
    let mut command = approval(&skipped, 42);
    command.env("PNS_SKIP_PHONE", "1");
    let output = hook_with(command, &skipped, "blocked", CLAUDE_APPROVAL);
    assert_eq!(output.status.code(), Some(42), "the operator's own answer");
    assert_eq!(
        submissions(&skipped),
        ["claude-hook"],
        "a suppressed card is not a suppressed prompt"
    );
    // THE ABSENT CARD IS NOT EVIDENCE THE VARIABLE WAS READ, the same trap
    // the channel-off sibling documents. The forward succeeded, so the blocked
    // path sets `PNS_SKIP_PHONE` itself and pns's phone leg is suppressed
    // whether or not the caller's variable ever reached anything. This line
    // pins that no second card appeared and nothing about what silenced it;
    // the submissions line above is this row's real pin, and the one the
    // `&& !overrides.skip_phone` mutation kills.
    assert!(!skipped.fired("mobile"));
}

#[test]
fn a_mute_never_touches_the_approval_a_blocked_operator_is_waiting_to_answer() {
    // THE EXEMPTION IS STRUCTURAL: the forward runs in `blocking_event`, which
    // builds its own overrides and never constructs a delivery plan, so the
    // mute cannot reach it. Structural means it can be broken by moving code
    // rather than by editing a line this feature added, which is what this
    // pins. A muted operator who blocks on a permission prompt still gets the
    // card and still answers it; only pns's own duplicate notification about
    // that block goes quiet.
    let sandbox = Sandbox::new("hook-blocked-muted");
    // The three stub channels named explicitly, plus the nag scheduled: the
    // second half of this test is the MIRROR case, and a nudge needs a schedule
    // to exist at all.
    sandbox.write_config(&nag_config(300));
    let mut command = with_state_dir(&sandbox);
    // Away, so the phone is the only way to answer at all.
    command.env("PNS_IDLE_SECS", "99999");
    sandbox.stub_moshi(&mut command, 42);
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    let expiry = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("a clock past 1970")
        .as_secs()
        + 600;
    let quiet_until = sandbox.path("state/quiet-until");
    std::fs::write(&quiet_until, format!("{expiry}\n")).expect("the mute");

    let payload = "{\"message\":\"may I\",\"session_id\":\"s1\"}\n";
    let output = hook_with(command, &sandbox, "blocked", payload);

    assert_eq!(
        output.status.code(),
        Some(42),
        "the exit code IS the operator's decision, muted or not"
    );
    assert_eq!(
        std::fs::read_to_string(sandbox.path("moshi.stdin")).expect("moshi read the payload"),
        payload,
        "byte for byte: a consumed-but-not-forwarded stream leaves moshi with an empty parse"
    );
    assert_eq!(
        std::fs::read_to_string(sandbox.path("moshi.argv"))
            .expect("moshi argv")
            .trim(),
        "claude-hook"
    );
    // The paper trail is written. The ABSENT CARD IS NOT EVIDENCE OF THE MUTE:
    // the unmuted control produces the same two legs, because the forward's own
    // skip suppresses pns's phone leg either way. This line pins that no second
    // card appeared, and nothing about what silenced it; the pins above are
    // what carry the exemption.
    assert!(sandbox.fired("hermes"), "the durable log is never muted");
    assert!(!sandbox.fired("mobile"));
    assert!(
        std::fs::read_to_string(&quiet_until)
            .expect("the mute survives")
            .trim()
            == expiry.to_string(),
        "the mute is untouched by the event it did not suppress"
    );

    // AND THE MIRROR CASE, extended here rather than written as a sibling: a
    // NUDGE about that same approval is INFORMATIONAL, so the mute that cannot
    // touch the approval holds the nudge's banner and phone card back
    // completely. The nudge is gated through the same `decide` call as any other
    // event rather than through a second rule, which is why this is one more
    // paragraph in this test instead of a second one.
    //
    // A SUPPRESSED NUDGE IS LOST, deliberately: nothing here is journaled for
    // replay, because a "still waiting" card replayed hours later about a
    // question long since answered is worse than silence.
    counted_channels(&sandbox);
    write_record(&sandbox, "s1", 300, "may I", "wW:p21");
    support::run(&mut nag(&sandbox));
    assert_eq!(
        deliveries(&sandbox, "macos-banner"),
        0,
        "a muted operator gets no banner about a nudge"
    );
    assert_eq!(
        deliveries(&sandbox, "mobile"),
        0,
        "and no phone card either: escalation is not an exemption"
    );
    assert_eq!(
        deliveries(&sandbox, "hermes"),
        1,
        "while the durable log is never muted, for a nudge any more than for the approval"
    );
}

#[test]
fn a_focus_never_touches_the_approval_a_blocked_operator_is_waiting_to_answer() {
    // A STRUCTURAL GUARD, and it says so about itself: it passes the moment
    // the field exists, and its whole job is to keep passing. `blocking_event`
    // decides the forward through `forward_to_moshi`, which reads the presence
    // probes and never constructs a delivery plan, so nothing on `Overrides`
    // can reach it. That guarantee breaks by MOVING code rather than by
    // editing a line this feature added, which no unit test can observe.
    //
    // THE NEAR DUPLICATE OF THE MUTE'S OWN TEST IS DELIBERATE. That one would
    // keep passing on the day a Focus started suppressing approvals, and this
    // is a safety property: an operator inside a Focus who blocks on a
    // permission prompt still gets the card and still answers it.
    let sandbox = Sandbox::new("hook-blocked-focus");
    let mut command = with_state_dir(&sandbox);
    // Away, so the phone is the only way to answer at all.
    command.env("PNS_IDLE_SECS", "99999");
    sandbox.stub_moshi(&mut command, 42);
    sandbox.write_focus_store("com.apple.sleep.sleep-mode", "Sleep");
    sandbox.write_config(
        "[plugins.mobile]\nenabled = true\ntype = \"moshi\"\n[plugins.hermes]\nenabled = true\n\
         [plugins.macos-banner]\nenabled = true\n[focus]\nsilence = [\"Sleep\"]\n",
    );

    let payload = "{\"message\":\"may I\",\"session_id\":\"s1\"}\n";
    let output = hook_with(command, &sandbox, "blocked", payload);

    assert_eq!(
        output.status.code(),
        Some(42),
        "the exit code IS the operator's decision, Focus or not"
    );
    assert_eq!(
        std::fs::read_to_string(sandbox.path("moshi.stdin")).expect("moshi read the payload"),
        payload,
        "byte for byte: a consumed-but-not-forwarded stream leaves moshi with an empty parse"
    );
    assert!(
        sandbox.fired("hermes"),
        "the durable log is never silenced by a Focus either"
    );
}
