use super::*;

#[test]
fn a_forwarded_gate_leaves_the_state_markers_untouched() {
    for (name, argv) in [("gate-markers-bare", vec!["pi-hook"])] {
        let sandbox = Sandbox::new(name);
        std::fs::create_dir_all(sandbox.state()).expect("private state");
        let existing = marker(&sandbox, "existing");
        std::fs::write(&existing, b"1700000000\n").expect("existing marker");
        let mut command = sandbox.pns_stateful();
        command.args(argv).env("PNS_SCREEN_IDLE", "99999");
        sandbox.stub_moshi(&mut command, 7);
        let mut child = captured_child::CapturedChild::spawn(&mut command).expect("gate runs");
        write_payload(&mut child.child, b"{\"session_id\":\"new-session\"}\n");
        // THE SHARED LIVENESS BOUND, not a bound of its own. Nothing below
        // reads the elapsed time: the row pins the exit code, the submission
        // and the untouched markers. Its own 800ms was the tightest bound in
        // this file and failed under load on a build that passes alone.
        let output = child
            .output_within(HANG_LIMIT)
            .expect("gate and pipe holders finish inside the bound");
        assert_eq!(output.status.code(), Some(7));
        assert_eq!(submissions(&sandbox), ["pi-hook"]);
        assert_eq!(
            std::fs::read(&existing).expect("existing marker survives"),
            b"1700000000\n"
        );
        let entries: Vec<_> = std::fs::read_dir(sandbox.state())
            .expect("state remains readable")
            .map(|entry| entry.expect("state entry").path())
            .collect();
        assert_eq!(entries, [existing], "a gate creates no state marker");
        for channel in ["mobile", "hermes", "banner"] {
            assert!(!sandbox.fired(channel), "a gate raised {channel}");
        }
    }
}

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
    command.env("PNS_SCREEN_IDLE", "99999");
    sandbox.stub_moshi(&mut command, 7);
    let mut child = command
        .args(argv)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the engine runs");
    write_payload(&mut child, payload.as_bytes());
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
    command.env("PNS_SCREEN_IDLE", "99999");
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
fn the_retired_gate_subcommand_is_refused_rather_than_forwarded() {
    // ONE SPELLING NOW. `pns gate <harness>-hook` is gone, and `gate` names no
    // subcommand, so it earns the usage text and exit 2 like any other typo
    // instead of a second way into one gate.
    let sandbox = Sandbox::new("gate-subcommand-retired");
    for argv in [
        vec!["gate", "pi-hook"],
        vec!["gate", "nonsense"],
        vec!["gate"],
    ] {
        let output = gate_argv(&sandbox, &argv, "{\"ask\":1}\n");
        assert_eq!(output.status.code(), Some(2), "argv {argv:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("pns: usage:"),
            "argv {argv:?} was refused in silence"
        );
        assert!(
            !sandbox.path("moshi.argv").exists(),
            "argv {argv:?} reached moshi"
        );
        assert!(!sandbox.fired("hermes"), "argv {argv:?} raised an event");
    }
}

#[test]
fn a_hook_shaped_word_the_gate_will_not_vouch_for_says_so_instead_of_exiting_zero() {
    // THE SILENT EXIT, which is the worst answer available here: a hook that
    // succeeds having forwarded nothing looks wired for the life of the
    // install. Every word below is hook-SHAPED, so it reaches the gate's own
    // judgement rather than the dispatcher's usage text, and the gate has to
    // both refuse it and say which word it refused.
    let sandbox = Sandbox::new("gate-word-refused-aloud");
    for word in ["Pi-hook", "-hook", "pi_hook-hook", "PI-hook"] {
        let output = gate(&sandbox, word, "{}");
        assert_eq!(output.status.code(), Some(2), "word {word:?}");
        let said = String::from_utf8_lossy(&output.stderr);
        assert!(
            said.contains("is not a harness word") && said.contains(word),
            "word {word:?} was refused in silence: {said:?}"
        );
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
    // HOOK-SHAPED WORDS ARE THE TWIN ABOVE'S, not this one's: a second copy
    // of a guard is not a second guard. These are the shapes that never reach
    // the gate at all, so the dispatcher's own refusal is what they pin.
    for (word, code) in [("../../etc/passwd", 2), ("pi-hook; rm -rf /", 2)] {
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
        .env("PNS_SCREEN_IDLE", "0")
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
    command.env("PNS_SCREEN_IDLE", "99999");
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
