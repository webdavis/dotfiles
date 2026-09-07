use super::*;

#[test]
fn at_the_desk_the_approval_is_never_forwarded_and_the_harness_prompts_as_usual() {
    let sandbox = Sandbox::new("hook-blocked-desk");
    let mut command = sandbox.pns();
    command.env("PNS_IDLE_SECS", "0");
    sandbox.stub_moshi(&mut command, 42);
    let output = hook_with(command, &sandbox, "blocked", r#"{"message":"may I"}"#);
    assert_eq!(output.status.code(), Some(0), "no opinion: prompt as usual");
    assert!(
        !sandbox.path("moshi.argv").exists(),
        "the operator is right here; the card would be noise"
    );
}

#[test]
fn a_phone_used_more_recently_than_the_desk_gets_the_approval_forwarded_to_it() {
    // THE GATE READS THE SAME ARBITRATION as the delivery plan, so the
    // phone-input amendment reaches it with no wiring of its own: the desk
    // was touched 90s ago and is still inside the freshness window, but the
    // phone was touched 5s ago and that is where the operator can answer.
    let sandbox = Sandbox::new("hook-blocked-phone-fresher");
    let mut command = sandbox.pns();
    command
        .env("PNS_IDLE_SECS", "90")
        .env("PNS_PHONE_INPUT_AGE", "5");
    sandbox.stub_moshi(&mut command, 42);
    let output = hook_with(command, &sandbox, "blocked", r#"{"message":"may I"}"#);
    assert_eq!(output.status.code(), Some(42), "the operator's own answer");
    assert!(sandbox.path("moshi.argv").exists());
}

#[test]
fn a_presence_reading_nobody_can_parse_still_forwards_the_approval() {
    // FAIL TOWARD THE PHONE, ACROSS THE FORWARD. `surface_reading` refuses a
    // garbled `PNS_IDLE_SECS` rather than falling back to a probe or to a
    // default, so the surface is not Desk and `forward_to_moshi` says yes.
    // The engine unit
    // `the_lock_probe_is_read_only_where_the_idle_probe_returned_a_reading`
    // pins that refusal on `operator_surface` itself and nothing crossed the
    // forward: a `forward_to_moshi` that returned false on an unreadable
    // reading would leave every unit green while an operator whose presence
    // could not be read lost approvals entirely. That is the failure on this
    // path that looks exactly like being at your desk, which is why the whole
    // section exists.
    //
    // MUTATION, measured: `surface_reading` reading an invalid idle override
    // as a desk just touched (`(Some(0), None)` in place of `(None, None)`)
    // kills this test.
    let sandbox = Sandbox::new("hook-blocked-idle-garbled");
    let mut command = approval(&sandbox, 42);
    command.env("PNS_IDLE_SECS", "soup");
    let output = hook_with(command, &sandbox, "blocked", CLAUDE_APPROVAL);
    assert_eq!(output.status.code(), Some(42), "the operator's own answer");
    assert_eq!(
        submissions(&sandbox),
        ["claude-hook"],
        "a presence nobody could read is not an operator sitting at the desk"
    );
}

#[test]
fn at_the_desk_a_blocked_approval_banners_a_hidden_pane_and_leaves_a_watched_one_alone() {
    // THE SURFACE SLICE 27 PUTS BUTTONS ON, composed end to end so that
    // changing it has to be said out loud. `plan`'s own matrix pins these two
    // rows as a unit and `tests/dispatch.rs` pins them through `--state
    // blocked`, but nothing anywhere composed `hook blocked` with a desk
    // reading and asked what the operator actually receives, which is the one
    // question an approve button on the banner changes the answer to.
    //
    // NEITHER ROW FORWARDS, the same rule
    // `at_the_desk_the_approval_is_never_forwarded_and_the_harness_prompts_as_usual`
    // states from the other side: the harness prompt is already in front of
    // them, so a card is noise and a round trip asks one question twice.
    //
    // MUTATION, measured: `plan`'s banner condition losing `!watching`
    // (`banner: surface == Surface::Desk`) kills the watched row here. Four
    // unit tests die on it too (`surface`'s own confirmed matrix and three in
    // `engine`), and that is the point rather than a duplication: those die on
    // the PLAN, and this dies on what a blocked hook puts in front of the
    // operator, which is the half the strike left unguarded.
    for (slug, label, pane_watched, banner_expected) in [
        ("hidden", "the pane is on another tab", false, true),
        (
            "watched",
            "the pane is the one being looked at",
            true,
            false,
        ),
    ] {
        let sandbox = Sandbox::new(&format!("hook-blocked-desk-{slug}"));
        let mut command = approval(&sandbox, 42);
        command
            .env("PNS_IDLE_SECS", "0")
            .env("HERDR_PANE_ID", "t1:p1");
        sandbox.stub_herdr(&mut command, pane_watched);
        let output = hook_with(command, &sandbox, "blocked", CLAUDE_APPROVAL);
        assert_eq!(output.status.code(), Some(0), "no round trip: {label}");
        assert!(
            submissions(&sandbox).is_empty(),
            "the prompt is already on their screen: {label}"
        );
        assert_eq!(
            sandbox.fired("macos-banner"),
            banner_expected,
            "the desk banner: {label}"
        );
        assert!(
            sandbox.fired("hermes"),
            "and the durable leg carries every approval: {label}"
        );
    }
}
