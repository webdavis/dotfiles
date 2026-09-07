use super::*;

// --- the gate, as a real process --------------------------------------------

/// The gate is reached by the BARE harness word, because moshi's generated
/// extension holds one pathname with no room for a subcommand.
fn gate(sandbox: &Sandbox, word: &str, payload: &str) -> std::process::Output {
    gate_argv(sandbox, &[word], payload)
}

/// The same gate, reached by whatever argv the caller spells: the bare word
/// moshi's extension uses, or the `gate <word>` form the documentation gives
/// an operator.
fn gate_argv(sandbox: &Sandbox, argv: &[&str], payload: &str) -> std::process::Output {
    let mut command = sandbox.pns();
    command.env("PNS_IDLE_SECS", "99999");
    sandbox.stub_moshi(&mut command, 7);
    let mut child = command
        .args(argv)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the engine runs");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(payload.as_bytes())
        .expect("payload");
    child.wait_with_output().expect("output")
}

#[test]
fn the_bare_harness_word_forwards_through_the_gate_and_returns_the_decision() {
    let sandbox = Sandbox::new("gate-forwards");
    let output = gate(&sandbox, "pi-hook", "{\"ask\":1}\n");
    assert_eq!(
        output.status.code(),
        Some(7),
        "the decision is the exit code"
    );
    assert_eq!(
        std::fs::read_to_string(sandbox.path("moshi.stdin")).expect("moshi read it"),
        "{\"ask\":1}\n"
    );
    assert_eq!(
        std::fs::read_to_string(sandbox.path("moshi.argv"))
            .expect("argv")
            .trim(),
        "pi-hook"
    );
}

#[test]
fn a_zero_decision_passes_through_as_zero_and_is_not_a_default() {
    let sandbox = Sandbox::new("gate-approves");
    let mut command = sandbox.pns();
    command.env("PNS_IDLE_SECS", "99999");
    sandbox.stub_moshi(&mut command, 0);
    let mut child = command
        .arg("pi-hook")
        .stdin(Stdio::piped())
        .spawn()
        .expect("runs");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(b"{}")
        .expect("payload");
    assert_eq!(child.wait().expect("wait").code(), Some(0));
    assert!(
        sandbox.path("moshi.argv").exists(),
        "an approval reaches moshi; a zero exit is its answer, not a skip"
    );
}

#[test]
fn the_documented_gate_subcommand_reaches_the_same_gate_as_the_bare_word() {
    // CLAUDE.md gives `pns gate <harness>-hook` as the operator-facing form,
    // and only the bare word was ever implemented: the documented one fell
    // through to EVENT mode, which forwarded nothing and fired a notification
    // about an empty event nobody asked for.
    let sandbox = Sandbox::new("gate-subcommand");
    let output = gate_argv(&sandbox, &["gate", "pi-hook"], "{\"ask\":1}\n");
    assert_eq!(
        output.status.code(),
        Some(7),
        "the decision is still the exit code"
    );
    assert_eq!(
        std::fs::read_to_string(sandbox.path("moshi.argv"))
            .expect("argv")
            .trim(),
        "pi-hook"
    );
    assert!(
        !sandbox.fired("hermes"),
        "a gate forwards; it never raises an event of its own"
    );
}

#[test]
fn the_gate_subcommand_refuses_a_word_it_will_not_vouch_for_without_notifying() {
    // The refusal has to be a refusal on BOTH forms. Falling through to event
    // mode here is how the bogus notification got out.
    let sandbox = Sandbox::new("gate-subcommand-refuses");
    for word in ["", "nonsense", "../../etc/passwd", "pi-hook; rm -rf /"] {
        let output = gate_argv(&sandbox, &["gate", word], "{}");
        assert_eq!(output.status.code(), Some(0), "word {word:?}");
        assert!(
            !sandbox.path("moshi.argv").exists(),
            "word {word:?} reached moshi"
        );
        assert!(!sandbox.fired("hermes"), "word {word:?} raised an event");
    }
}

#[test]
fn a_shape_the_gate_will_not_vouch_for_is_never_handed_to_moshi() {
    let sandbox = Sandbox::new("gate-refuses");
    for (word, code) in [
        ("../../etc/passwd", 2),
        ("pi-hook; rm -rf /", 2),
        ("Pi-hook", 2),
        // A leading `-` used to be a free pass into the producer contract's
        // empty event, so a mistyped harness word delivered in silence. It is
        // now the operator's rule, not a regression: `-hook` names no flag
        // this parser recognizes, so it is refused like any other typo.
        ("-hook", 2),
    ] {
        let output = gate(&sandbox, word, "{}");
        assert_eq!(output.status.code(), Some(code), "word {word:?}");
        assert!(
            !sandbox.path("moshi.argv").exists(),
            "word {word:?} reached moshi"
        );
    }
}

#[test]
fn at_the_desk_the_gate_submits_nothing_and_exits_zero() {
    // THE GATE IS PRESENCE-GATED TOO, off the same reading the hook path and
    // the delivery plan take. Every other gate test states the away clock, so
    // the gate's own reading has never been exercised at all: a build that
    // dropped it would card a phone for a prompt the operator is sitting in
    // front of, and every gate test would stay green. The Command is built
    // here rather than through `gate_argv`, which hard-codes away, so no
    // existing test moves.
    //
    // MECHANISM-BOUND, IN THE DANGEROUS DIRECTION: the absence reads through
    // `submissions`, so item 25 re-points one function rather than leaving a
    // desk-side submission unguarded behind a filename that no longer exists.
    let sandbox = Sandbox::new("gate-desk");
    let mut command = sandbox.pns();
    command
        .env("PNS_IDLE_SECS", "0")
        .env("PNS_PHONE_INPUT_AGE", "99999");
    sandbox.stub_moshi(&mut command, 7);
    let mut child = spawn_gate(command, "pi-hook");
    // The pipe is closed rather than written through: a gate that declines
    // never reads its stdin, so a write is allowed to go nowhere.
    write_payload(&mut child, b"{\"ask\":1}\n");
    assert_eq!(
        finished_within(child, HANG_LIMIT),
        Some(0),
        "no opinion: the harness prompts as usual"
    );
    assert!(
        submissions(&sandbox).is_empty(),
        "the operator is right here; the card would be noise"
    );
    assert!(
        !sandbox.fired("hermes"),
        "a gate that declines raises no event of its own either"
    );
}

#[test]
fn the_gate_refuses_an_over_cap_payload_as_firmly_as_the_hook_does() {
    // The reader caps stdin, so an over-cap payload arrives CUT MID-OBJECT,
    // and handing that on is the empty parse the byte-for-byte contract exists
    // to prevent. The check runs at BOTH entry points and either call site can
    // lose it independently; only the hook's was pinned. Truncated JSON is the
    // same empty parse over any transport, so the invariant outlives the pipe.
    //
    // MECHANISM-BOUND, IN THE DANGEROUS DIRECTION: the absence reads through
    // `submissions` for the reason the desk twin above states.
    let sandbox = Sandbox::new("gate-oversized");
    let mut command = sandbox.pns();
    command.env("PNS_IDLE_SECS", "99999");
    sandbox.stub_moshi(&mut command, 42);
    let mut child = spawn_gate(command, "pi-hook");
    let payload = format!(r#"{{"ask":"{}"}}"#, "x".repeat(1_200_000));
    write_payload(&mut child, payload.as_bytes());
    assert_eq!(
        finished_within(child, HANG_LIMIT),
        Some(0),
        "an over-cap payload is not the operator's decision"
    );
    assert!(
        submissions(&sandbox).is_empty(),
        "half an object must never reach moshi"
    );
}

#[test]
fn the_gate_submits_one_prompt_exactly_once() {
    // THE OTHER SUBMITTER. Single-submitter is a rule about the PROMPT rather
    // than about one entry point, and the gate is the half pi and omp reach
    // directly with no pns hook in front of it. A second spawn here is a
    // second card and a second answer to one question, and until this counted
    // them nothing in the crate would have said so.
    //
    // MECHANISM-BOUND: the count is read off the submission record, so this
    // goes RED at the endpoint switch for item 25 to rewrite.
    let sandbox = Sandbox::new("gate-single-submitter");
    let output = gate(&sandbox, "pi-hook", "{\"ask\":1}\n");
    assert_eq!(
        output.status.code(),
        Some(7),
        "the decision is still the exit code"
    );
    assert_eq!(
        submissions(&sandbox),
        ["pi-hook"],
        "one prompt, one submission: a second card is a second answer nobody gave"
    );
}

/// The gate as a real process, reached by the bare harness word, with the
/// payload still to be written. The twin of `spawn_hook` for the other entry
/// point.
fn spawn_gate(mut command: Command, word: &str) -> std::process::Child {
    command
        .arg(word)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("the engine runs")
}
