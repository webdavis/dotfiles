use super::*;

#[test]
fn a_harness_word_is_refused_as_usage_and_never_reaches_moshi() {
    // moshi's pi and omp extensions write to its socket themselves, so a
    // harness word names no pns command and earns the usage text and exit 2
    // like any other typo.
    let sandbox = Sandbox::new("harness-word-refused");
    for argv in [
        vec!["pi-hook"],
        vec!["omp-hook"],
        vec!["claude-hook"],
        vec!["gate", "pi-hook"],
    ] {
        let mut command = sandbox.pns();
        command.env("PNS_SCREEN_IDLE", "99999");
        sandbox.stub_moshi(&mut command, 7);
        let mut child = command
            .args(&argv)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the engine runs");
        write_payload(&mut child, b"{\"ask\":1}\n");
        let output = child.wait_with_output().expect("output");
        assert_eq!(output.status.code(), Some(2), "argv {argv:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("pns: usage:"),
            "argv {argv:?} was refused in silence"
        );
        assert!(
            submissions(&sandbox).is_empty(),
            "argv {argv:?} reached moshi"
        );
        assert!(!sandbox.fired("hermes"), "argv {argv:?} raised an event");
    }
}
