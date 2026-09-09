use super::*;

#[test]
fn piped_input_is_complete_and_closed_before_the_reply_finishes() {
    let input = vec![b'x'; 128 * 1024];
    let mut runner = SystemRunner::new(Duration::from_millis(600));
    let result = runner.run_completed(Path::new("/bin/cat"), &[], CommandIo::Input(&input));
    let output = result.expect("complete request reply");
    assert_eq!(output.exit, 0);
    assert_eq!(output.bytes.len(), input.len());
    assert!(output.bytes == input, "input bytes changed");
}

#[test]
fn output_backpressure_is_drained_while_input_is_still_pending() {
    let input = vec![b'x'; 128 * 1024];
    let mut runner = SystemRunner::new(Duration::from_millis(600));
    let output = runner
        .run_completed(
            Path::new("/bin/sh"),
            &[
                OsStr::new("-c"),
                OsStr::new("/bin/dd if=/dev/zero bs=4096 count=24 2>/dev/null; /bin/cat; exit 42"),
            ],
            CommandIo::Input(&input),
        )
        .expect("concurrent input/output");
    assert_eq!(output.exit, 42);
    assert_eq!(&output.bytes[..96 * 1024], vec![0; 96 * 1024]);
    assert_eq!(output.bytes.len(), 96 * 1024 + input.len());
    assert!(output.bytes[96 * 1024..] == input, "pending input changed");
}

#[test]
fn an_empty_input_is_closed_so_the_child_can_finish() {
    let mut runner = SystemRunner::new(Duration::from_millis(300));
    assert_eq!(
        runner.run_completed(Path::new("/bin/cat"), &[], CommandIo::Input(b"")),
        Ok(CommandOutput {
            bytes: vec![],
            exit: 0
        })
    );
}

#[test]
fn a_child_that_never_reads_input_is_killed_with_its_descendants() {
    super::lifecycle::timed_probe_with(
        "/bin/sh -c 'trap \"\" TERM; while :; do :; done' & printf '%s\\n%s\\n' \"$$\" \"$!\" >\"$1\"; wait",
        CommandIo::Input(&[b'x'; 128 * 1024]),
    );
}
