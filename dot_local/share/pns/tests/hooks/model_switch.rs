use super::*;

// --- observation mode: the automatic model-switch arm ----------------------
//
// `Attempt::Observation` is the third attempt path (main.rs): an occurrence
// the operator should hear about that changes no workflow or marker state.
// D4's `auto` arm (a `PostModelSwitch` event whose `source` is `auto`) is its
// first caller. Every test here plants its own precondition and asserts the
// stub channel fired INSIDE it: an arm that never reaches `run_event` would
// leave every marker-neutral file unchanged too, which would make the
// negative assertions pass for the wrong reason.

pub(crate) fn model_switch_payload(session: &str, source: &str) -> String {
    format!(
        r#"{{"session_id":"{session}","cwd":"/a/dotfiles","from_model":"claude-sonnet-4-5","to_model":"claude-opus-4-6","source":"{source}"}}"#
    )
}

#[test]
fn an_observation_still_delivers_and_is_logged() {
    let sandbox = Sandbox::new("observation-delivers-and-logs");
    sandbox.write_config(&nag_config(300));
    counted_channels(&sandbox);

    // THE MODEL NAMES CARRY CONTROL BYTES (a BEL, a CRLF), which the card
    // must scrub through the same filter every other rendered field passes:
    // a payload field reaches a banner and a Discord message verbatim
    // otherwise, and the harness is not the only thing that can write one.
    let output = hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "model-switch",
        r#"{"session_id":"s1","cwd":"/a/dotfiles","from_model":"claude-sonnet-4-5\u0007","to_model":"claude-opus-4-6\r\n","source":"auto"}"#,
    );

    assert!(output.status.success(), "an observation still exits 0");
    assert_eq!(
        deliveries(&sandbox, "hermes"),
        1,
        "the positive control: one card"
    );
    let event = sandbox.event("hermes");
    assert_eq!(event["state"], "model-switch");
    assert_eq!(event["agent"], "claude");
    assert_eq!(
        event["detail"],
        "automatic session model change: claude-sonnet-4-5 to claude-opus-4-6"
    );
    let recorded =
        std::fs::read_to_string(sandbox.path("state/decisions")).expect("the decision ring");
    let lines: Vec<&str> = recorded.lines().collect();
    assert_eq!(lines.len(), 1, "one event, one line: {recorded:?}");
    assert!(
        lines[0].contains(" claude/model-switch "),
        "the record names the harness and the state: {recorded:?}"
    );
    assert!(
        lines[0].contains(" nag=no "),
        "an observation is logged with no nag: {recorded:?}"
    );
}

#[test]
fn an_auto_switch_between_equal_names_delivers_nothing() {
    // SOL 1: `source == "auto"` alone cannot tell a real switch from a
    // harness re-announcing the model it was already on, so "opus to opus"
    // must not become a card.
    let sandbox = Sandbox::new("observation-equal-names-silent");
    sandbox.write_config(&nag_config(300));
    counted_channels(&sandbox);

    let output = hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "model-switch",
        r#"{"session_id":"s1","from_model":"claude-opus-4-6","to_model":"claude-opus-4-6","source":"auto"}"#,
    );

    assert!(output.status.success());
    assert_eq!(
        deliveries(&sandbox, "hermes"),
        0,
        "opus to opus is not a transition worth a card"
    );
}

#[test]
fn an_auto_switch_missing_a_model_name_delivers_nothing() {
    // SOL 1: a missing field becomes empty in the payload parser, and an
    // empty name on either side is not a transition either.
    let sandbox = Sandbox::new("observation-missing-model-silent");
    sandbox.write_config(&nag_config(300));
    counted_channels(&sandbox);

    let output = hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "model-switch",
        r#"{"session_id":"s1","from_model":"claude-opus-4-6","source":"auto"}"#,
    );

    assert!(output.status.success());
    assert_eq!(
        deliveries(&sandbox, "hermes"),
        0,
        "a missing to_model has nothing on one side of the arrow"
    );
}

#[test]
fn an_auto_switch_strips_a_unicode_format_character_from_the_name() {
    // SOL 1: `flattened` strips whitespace and `char::is_control` (the Cc
    // set) but not Cf, so a right-to-left override survived it and could
    // reorder the rendered line. It must not reach the card.
    let sandbox = Sandbox::new("observation-invisible-character-stripped");
    sandbox.write_config(&nag_config(300));
    counted_channels(&sandbox);

    let output = hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "model-switch",
        "{\"session_id\":\"s1\",\"from_model\":\"claude-sonnet-4-5\",\"to_model\":\"claude-opus\u{202e}-4-6\",\"source\":\"auto\"}",
    );

    assert!(output.status.success());
    assert_eq!(deliveries(&sandbox, "hermes"), 1);
    let event = sandbox.event("hermes");
    assert_eq!(
        event["detail"], "automatic session model change: claude-sonnet-4-5 to claude-opus-4-6",
        "the override character is gone from the rendered name"
    );
}

#[test]
fn a_non_auto_model_switch_source_delivers_nothing_and_writes_nothing() {
    // S2: THIS TEST IS VACUOUS ALONE. An unknown hook word exits 0 and writes
    // nothing (the catch-all arm), so "a non-auto source delivers nothing"
    // would be true even with no `model-switch` arm at all. Prove `auto`
    // fires FIRST, on this same sandbox, then prove every other documented
    // source leaves every trace byte-identical to that snapshot.
    let sandbox = Sandbox::new("observation-non-auto-source-silent");
    sandbox.write_config(&nag_config(300));
    counted_channels(&sandbox);

    let output = hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "model-switch",
        &model_switch_payload("s1", "auto"),
    );
    assert!(output.status.success(), "auto still exits 0");
    assert_eq!(deliveries(&sandbox, "hermes"), 1, "auto delivers");
    let deliveries_after_auto = deliveries(&sandbox, "hermes");
    let decisions_after_auto =
        std::fs::read_to_string(sandbox.path("state/decisions")).unwrap_or_default();
    let activity_after_auto = state_lines(&sandbox, "activity");
    let present_after_auto =
        std::fs::read_to_string(sandbox.path("state/last-present")).unwrap_or_default();

    // The four other documented words, then the shapes the reference does
    // not list: a `source` that is missing, empty, of the wrong type, spelled
    // in the wrong case, or a word it never documents. Each reads as not
    // `auto`, and not `auto` is silence; a gate that let an ABSENT source
    // through would fire on every harness that sends none.
    let documented = ["command", "picker", "sdk", "resume"]
        .map(|source| (source.to_string(), model_switch_payload("s2", source)));
    let unlisted = [
        (
            "missing",
            r#"{"session_id":"s2","from_model":"a","to_model":"b"}"#,
        ),
        (
            "empty",
            r#"{"session_id":"s2","from_model":"a","to_model":"b","source":""}"#,
        ),
        (
            "a number",
            r#"{"session_id":"s2","from_model":"a","to_model":"b","source":7}"#,
        ),
        (
            "AUTO",
            r#"{"session_id":"s2","from_model":"a","to_model":"b","source":"AUTO"}"#,
        ),
        (
            "manual",
            r#"{"session_id":"s2","from_model":"a","to_model":"b","source":"manual"}"#,
        ),
    ]
    .map(|(label, payload)| (label.to_string(), payload.to_string()));
    for (source, payload) in documented.into_iter().chain(unlisted) {
        let output = hook_with(with_state_dir(&sandbox), &sandbox, "model-switch", &payload);
        assert!(output.status.success(), "{source}: still exits 0");
        assert_eq!(
            deliveries(&sandbox, "hermes"),
            deliveries_after_auto,
            "{source}: delivers nothing"
        );
        assert_eq!(
            std::fs::read_to_string(sandbox.path("state/decisions")).unwrap_or_default(),
            decisions_after_auto,
            "{source}: writes no decision line"
        );
        assert_eq!(
            state_lines(&sandbox, "activity"),
            activity_after_auto,
            "{source}: writes no activity line"
        );
        assert_eq!(
            std::fs::read_to_string(sandbox.path("state/last-present")).unwrap_or_default(),
            present_after_auto,
            "{source}: moves no presence edge"
        );
    }
}
