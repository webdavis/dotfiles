use super::*;

/// One child writing `bytes` zeroes, read under a 4096-byte ceiling.
///
/// THE ONLY TESTS HERE THAT SPAWN, and they have to: what they pin is the
/// reader, and the reader only exists around a real pipe. Every other test
/// in this module drives fixture text through a parser, because the parsers
/// are the part a suite must not take off the live machine.
fn read_under_a_4096_byte_cap(bytes: usize) -> Option<String> {
    let mut command = std::process::Command::new("/bin/sh");
    command.args(["-c", &format!("head -c {bytes} /dev/zero; exit 0")]);
    run_bounded(command, None, Duration::from_secs(5), 4096)
}

#[test]
fn a_command_that_talks_past_the_cap_is_no_answer_rather_than_a_truncated_one() {
    // WITHOUT THE BOUND THIS READS THE LOT. A `read_to_end` into a growing
    // `Vec` is bounded by the DEADLINE alone, so a child that streams for
    // its whole window hands back everything it wrote and the caller learns
    // the size only once it is all in memory. The summarizer is exactly
    // that child: an operator-named command running a model for minutes.
    //
    // AND A TRUNCATED ANSWER IS WORSE THAN NO ANSWER, which is why the cap
    // refuses in the DEADLINE'S OWN DIRECTION rather than handing back what
    // it managed to read. Cut at the ceiling, a process list loses its last
    // rows and a JSON listing stops mid-object, and both arrive looking
    // exactly like a complete short answer. Every caller reads no answer as
    // unknown, and unknown never suppresses.
    assert_eq!(read_under_a_4096_byte_cap(100_000), None);
}

#[test]
fn a_short_answer_is_told_apart_from_the_cap_because_it_is_still_an_answer() {
    // The other half of the same statement: the refusal above has to be the
    // CAP and not the reader giving up on anything large-ish, so a child
    // that stops on its own is read whole, and the ceiling itself is a
    // working answer rather than the first refused one.
    assert_eq!(
        read_under_a_4096_byte_cap(12).map(|read| read.len()),
        Some(12)
    );
    assert_eq!(
        read_under_a_4096_byte_cap(4096).map(|read| read.len()),
        Some(4096),
        "the ceiling is inclusive, as every other bound in this crate is"
    );
}

#[test]
fn the_production_runner_captures_stdout_on_success() {
    assert_eq!(
        SystemCommandRunner.run("/bin/echo", &["ok"]),
        Some("ok\n".to_string())
    );
}

#[test]
fn the_production_runner_yields_no_reading_from_a_failing_command() {
    // Partial output from a failed command must never be parsed as a live
    // reading; None is the unknown every consumer fails safe on.
    assert_eq!(SystemCommandRunner.run("/usr/bin/false", &[]), None);
}

#[test]
fn the_production_runner_yields_no_reading_for_a_missing_binary() {
    assert_eq!(
        SystemCommandRunner.run("/nonexistent/pns-no-such-binary", &[]),
        None
    );
}

#[test]
fn the_production_runner_keeps_a_reading_with_stray_invalid_bytes() {
    // Every reading is judged line by line downstream, so one bad byte
    // must cost its own line rather than the whole answer.
    let out = SystemCommandRunner
        .run("/bin/sh", &["-c", "printf 'a\\377b'"])
        .expect("stray bytes must not discard the reading");
    assert!(out.starts_with('a') && out.ends_with('b'));
}
