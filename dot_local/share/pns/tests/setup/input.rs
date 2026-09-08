use super::*;

#[test]
fn a_non_utf8_paste_is_reported_as_a_read_failure_rather_than_the_answers_ending() {
    let sandbox = Sandbox::without_config("setup-non-utf8");
    let mut pty = Pty::open();
    let mut child = pty.spawn(sandbox.bare().args(["setup"]));

    pty.read_until("or press enter to pair later: ", PTY_DEADLINE)
        .expect("the first prompt");
    // ISTRIP IS OFF ON THIS PTY, so a byte outside plain ASCII passes
    // through unmangled to `read_line`, which is where the crate's ONLY
    // non-UTF-8 read failure lives; a bare `\xff` is not valid UTF-8 on its
    // own or paired with anything that follows it.
    pty.write_all(&[0xFF, b'\n']);

    pty.read_to_eof(PTY_DEADLINE).expect("the wizard exits");
    let status = child.wait().expect("the wizard is reaped");
    assert_eq!(status.code(), Some(2), "transcript: {:?}", pty.transcript);
    assert!(
        pty.transcript.contains("the answers could not be read"),
        "the real reason was not reported: {:?}",
        pty.transcript
    );
    // THE DETAIL, not only the generic prefix: the underlying io::Error's
    // own text is "stream did not contain valid UTF-8", and a build that
    // reports the same generic prefix for every read failure would still
    // pass without this.
    assert!(
        pty.transcript.contains("valid UTF-8"),
        "the UTF-8 detail was not carried into the refusal: {:?}",
        pty.transcript
    );
    assert!(
        !pty.transcript
            .contains("the answers ended before the walk did"),
        "a read failure was reported as the input ending: {:?}",
        pty.transcript
    );
}
