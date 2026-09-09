use super::*;

#[test]
fn a_secret_typed_into_setup_never_reaches_the_pty_output() {
    // EVERY BRANCH THAT ASKS FOR A SECRET IS ARMED HERE, each with its own
    // unique value: a test that only walked the token (as this one used to)
    // cannot tell `armed_secret` from `armed` on the hermes, hue or router
    // branch, since either one composes a config that still looks right.
    const TOKEN: &str = "do-not-echo-this-token";
    const HERMES_KEY: &str = "do-not-echo-this-hermes-key";
    const HUE_KEY: &str = "do-not-echo-this-hue-key";
    const ROUTER_KEY: &str = "do-not-echo-this-router-key";
    let secrets = [TOKEN, HERMES_KEY, HUE_KEY, ROUTER_KEY];

    let sandbox = Sandbox::without_config("setup-hidden-secrets");
    let mut pty = Pty::open();
    let mut child = pty.spawn(sandbox.bare().args(["setup"]));

    // EACH ANSWER IS WRITTEN ONLY AFTER ITS OWN PROMPT IS VISIBLE: arming the
    // hidden read discards whatever is already queued (`TCSAFLUSH`), so
    // typing ahead of a prompt this walk has not printed yet would be lost.
    pty.read_until("or press enter to pair later: ", PTY_DEADLINE)
        .expect("the first prompt");
    // ECHO IS ALREADY OFF BY THE TIME THE PROMPT IS VISIBLE: the guard arms
    // before the prompt prints, so a secret typed the instant the prompt
    // appears cannot land in a still-echoing queue.
    assert_eq!(
        pty.tcgetattr().c_lflag & libc::ECHO,
        0,
        "the secret prompt was visible while echo was still on: {:?}",
        pty.transcript
    );
    pty.write_all(format!("{TOKEN}\n").as_bytes());

    pty.read_until("Post every event to hermes", PTY_DEADLINE)
        .expect("the hermes question");
    pty.write_all(b"y\n");
    pty.read_until("the signing key that route verifies: ", PTY_DEADLINE)
        .expect("the hermes key prompt");
    pty.write_all(format!("{HERMES_KEY}\n").as_bytes());

    pty.read_until("Flash hue lights", PTY_DEADLINE)
        .expect("the hue question");
    pty.write_all(b"y\n");
    pty.read_until("the hue bridge's address on the network: ", PTY_DEADLINE)
        .expect("the hue bridge prompt");
    pty.write_all(b"10.0.0.5\n");
    pty.read_until("an API key the bridge issued: ", PTY_DEADLINE)
        .expect("the hue key prompt");
    pty.write_all(format!("{HUE_KEY}\n").as_bytes());
    pty.read_until("the rooms to flash, comma separated", PTY_DEADLINE)
        .expect("the hue rooms prompt");
    pty.write_all(b"Kitchen,Office\n");

    pty.read_until("home wifi", PTY_DEADLINE)
        .expect("the router question");
    pty.write_all(b"y\n");
    pty.read_until("Which router backend?", PTY_DEADLINE)
        .expect("the router backend prompt");
    pty.write_all(b"unifi\n");
    pty.read_until("the router's URL: ", PTY_DEADLINE)
        .expect("the router URL prompt");
    pty.write_all(b"http://192.168.1.1\n");
    pty.read_until("an API key the router issued: ", PTY_DEADLINE)
        .expect("the router key prompt");
    pty.write_all(format!("{ROUTER_KEY}\n").as_bytes());
    pty.read_until("the phone's hostname on that router: ", PTY_DEADLINE)
        .expect("the router hostname prompt");
    pty.write_all(b"my-phone\n");

    pty.read_until("macOS Focus", PTY_DEADLINE)
        .expect("the focus question");
    pty.write_all(b"y\n");
    pty.read_until("which Focus modes mean it, comma separated: ", PTY_DEADLINE)
        .expect("the focus modes prompt");
    pty.write_all(b"Work,Sleep\n");

    pty.read_until("approval left unanswered", PTY_DEADLINE)
        .expect("the nag question");
    pty.write_all(b"y\n");

    pty.read_to_eof(PTY_DEADLINE).expect("the wizard exits");
    let status = child.wait().expect("the wizard is reaped");
    assert_eq!(status.code(), Some(0), "transcript: {:?}", pty.transcript);

    for secret in secrets {
        assert!(
            !pty.transcript.contains(secret),
            "a secret reached the pty output: {secret:?}: {:?}",
            pty.transcript
        );
    }
    // THE ECHOED ANSWER, not a bare `y`: the preamble already carries that
    // letter, so only the prompt's own tail followed by the typed answer and
    // the driver's echo of its Enter says echo was back on for an ordinary
    // question.
    assert!(
        pty.transcript.contains("[y/N]: y\r\n"),
        "an ordinary, non-secret answer stopped echoing too: {:?}",
        pty.transcript
    );
    // ECHONL IS WHAT PRODUCES THIS, with echo itself off: the prompt's own
    // ": " is immediately followed by the driver's echo of the typed Enter,
    // and then the next question, with no echoed secret in between.
    assert!(
        pty.transcript.contains(": \r\nPost every event"),
        "the hidden prompt's Enter was not echoed via ECHONL: {:?}",
        pty.transcript
    );

    let published = sandbox.root.join(".config/pns/config.toml");
    let contents = std::fs::read_to_string(&published).expect("the published config");
    for secret in secrets {
        assert!(
            contents.contains(secret),
            "a secret did not reach the file: {secret:?}: {contents}"
        );
    }
    let mode = std::fs::metadata(&published)
        .expect("the published config")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600, "the config is not the operator's alone");

    // THE GUARD MUST HAVE DROPPED, restoring the terminal it borrowed, even
    // though the child has already exited: a pty's master keeps reporting
    // the slave's last settings after the slave side closes.
    let after_exit = pty.tcgetattr();
    assert_ne!(
        after_exit.c_lflag & libc::ECHO,
        0,
        "echo was not restored once the wizard exited"
    );
}

#[test]
fn a_signal_sent_during_the_hidden_read_is_held_until_the_guard_drops() {
    // KILL, THEN OBSERVE, rather than a pty-level tty-stop test: SIGINT is
    // not a tty-stop signal and `Pty::spawn` gives the child a default
    // disposition and an empty mask, so the mask this observes is the
    // wizard's own rather than whatever launched the suite. Rust itself
    // does NOT do that: it hands a child the parent's mask verbatim.
    // `ps -o sigmask` reads 0 for a blocked
    // process on macOS, so a live process cannot be asked directly; sending
    // a real SIGINT and watching when it lands is the only external read
    // left. SIGTTIN is NOT covered here: a pending tty-stop signal is
    // discarded rather than delivered once the process group is orphaned,
    // and this harness (like CI) starts as its own session leader, so a
    // `waitpid(WUNTRACED)` on it would hang to the deadline instead of
    // observing anything. SIGPIPE is not covered either: the Rust runtime
    // sets it to `SIG_IGN` before `main`, so a delivered one ends nothing
    // and there is no moment of delivery to observe. Both are reviewed
    // rather than pinned.
    //
    // EVERY SIGNAL A PLAIN `kill` CAN DELIVER RIDES THE SAME WALK. Five of
    // the nine end the process by default and are observable this way, so
    // dropping any one of them from the guard's array is caught here rather
    // than only by review.
    use std::os::unix::process::ExitStatusExt;

    for (name, signal) in [
        ("setup-signal-pending-interrupt", libc::SIGINT),
        ("setup-signal-pending-alarm", libc::SIGALRM),
        ("setup-signal-pending-terminate", libc::SIGTERM),
        ("setup-signal-pending-quit", libc::SIGQUIT),
        ("setup-signal-pending-hangup", libc::SIGHUP),
    ] {
        let sandbox = Sandbox::without_config(name);
        let mut pty = Pty::open();
        let mut child = pty.spawn(sandbox.bare().args(["setup"]));

        pty.read_until("or press enter to pair later: ", PTY_DEADLINE)
            .expect("the first prompt");

        // THE GUARD IS ALREADY ARMED HERE, so a correct build holds this
        // rather than acting on it immediately.
        let sent = unsafe { libc::kill(child.id() as libc::pid_t, signal) };
        assert_eq!(sent, 0, "kill: {}", std::io::Error::last_os_error());
        // NO POLL, NO SLEEP: on a correct build the signal is blocked, so
        // the child is deterministically still alive the instant after
        // `kill` returns, rather than something that has to be waited out.
        assert!(
            matches!(child.try_wait(), Ok(None)),
            "the child died from signal {signal}, which the guard should still be holding"
        );

        pty.write_all(b"do-not-echo-this-token\n");
        pty.read_to_eof(PTY_DEADLINE).expect("the wizard exits");
        let status = child.wait().expect("the wizard is reaped");

        // THE HELD SIGNAL LANDS ONCE THE GUARD DROPS: `Drop` restores the
        // terminal before it unblocks the mask, so the pending signal is
        // delivered only after echo is already back on, and it is what ends
        // the process rather than a normal exit.
        assert_eq!(
            status.signal(),
            Some(signal),
            "the held signal {signal} was not delivered once the guard dropped: {:?}",
            pty.transcript
        );
        let after_exit = pty.tcgetattr();
        assert_ne!(
            after_exit.c_lflag & libc::ECHO,
            0,
            "the terminal was not restored before signal {signal} was delivered"
        );
    }
}
