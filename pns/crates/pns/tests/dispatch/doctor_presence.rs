use super::*;

#[test]
fn the_doctor_reads_the_room_off_the_state_file_and_judges_it_against_the_configs_own_rooms() {
    // THE ONE PLACE THE PURE HALVES ARE COMPOSED, and until this test the one
    // place nothing covered: the units above pin the parse and the policy as
    // functions, while the state file's own NAME, the read, and the config's
    // rooms and stale bound reaching `classify` live in the composition root.
    // Replacing that whole body with a constant `unknown (no reading)` left
    // every other test in this crate green.
    let sandbox = Sandbox::new("doctor-presence-reading");
    sandbox.write_config(&format!(
        "[plugins.hue]\nenabled = true\nbridge = \"{DEAD_BRIDGE}\"\nkey = \"k\"\n\
         [plugins.presence]\nenabled = true\ntype = \"hue\"\nrooms = [\"3F - Studio\"]\n"
    ));
    std::fs::create_dir_all(sandbox.state()).expect("the state directory");
    let published = sandbox.state().join("presence");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("a clock after 1970")
        .as_secs();

    // MOTION REPORTED NOW, so the printed age is zero however long this test
    // takes and the assertion is not a race against the clock.
    std::fs::write(&published, format!("{now} {now} 1 3F - Studio\n")).expect("the reading");
    let reported = stdout(&doctor_command(&sandbox).output().expect("the engine runs"));
    assert!(
        reported.contains("presence: 3F - Studio (0s ago)"),
        "the published room never reached the report: {reported}"
    );

    // AND THE OPERATOR'S OWN LIST IS WHAT ADMITS IT. A room nobody listed is
    // not a claim about where they are, so the same fresh line reads unknown.
    std::fs::write(&published, format!("{now} {now} 1 3F - Hallway\n")).expect("the reading");
    let reported = stdout(&doctor_command(&sandbox).output().expect("the engine runs"));
    assert!(
        reported.contains("presence: unknown (the reported room is not one this config watches)"),
        "a room the config never watched was reported as one: {reported}"
    );

    // AND A WRITER THAT STOPPED IS UNKNOWN, which is the whole guarantee: the
    // bound the config states has to reach the judgement, or a dead bridge
    // pins the operator in the last room it saw them in for good.
    let polled_at = now - 60;
    std::fs::write(
        &published,
        format!("{polled_at} {polled_at} 1 3F - Studio\n"),
    )
    .expect("the reading");
    // The doctor's clock can cross a second boundary after this fixture's
    // timestamp. Its reported age must fall between the two actual reads,
    // without weakening the stale verdict or accepting an unrelated unknown.
    let before = now_secs();
    let reported = stdout(&doctor_command(&sandbox).output().expect("the engine runs"));
    let after = now_secs();
    let line = report_rows(&reported)
        .into_iter()
        .find(|line| line.starts_with("presence: "))
        .expect("the presence result");
    let age = line
        .strip_prefix("presence: unknown (stale, poll ")
        .and_then(|age| age.strip_suffix("s old)"))
        .and_then(|age| age.parse::<u64>().ok())
        .unwrap_or_else(|| panic!("a poll nobody refreshed still named a room: {reported}"));
    assert_eq!(
        line,
        format!("presence: unknown (stale, poll {age}s old)"),
        "the stale poll age must use the report's exact grammar"
    );
    assert!(
        (before - polled_at..=after - polled_at).contains(&age),
        "poll age {age} is outside the observed bounds {}..={}",
        before - polled_at,
        after - polled_at
    );
}

#[test]
fn a_doctor_given_any_extra_word_prints_usage_exits_two_and_reaches_no_channel() {
    // A DOCTOR THAT QUIETLY IGNORED AN ARGUMENT is a check the operator
    // believes was narrower or wider than it was, which is worse than no check
    // at all. The empty word is in the set because a shell that expanded a
    // variable to nothing still typed something.
    for arguments in [
        vec!["extra"],
        vec!["send"],
        vec!["--dry-run"],
        vec![""],
        vec!["send", "hermes"],
    ] {
        let sandbox = Sandbox::new("doctor-refusal");
        sandbox.write_config(EVERY_DISPATCHED_CHANNEL);
        let mut command = doctor_command(&sandbox);
        // A RECORDING moshi-hook rather than the absent one, so "reaches no
        // channel" covers the binary the pairing check spawns as well: an
        // absent path proves nothing about whether it was reached for.
        stub_moshi_hook(
            &sandbox,
            &mut command,
            PAIRED_STATUS_JSON,
            PAIRED_STATUS_PLAIN,
        );
        let output = command.args(&arguments).output().expect("the engine runs");

        assert_eq!(
            output.status.code(),
            Some(2),
            "arguments: {arguments:?}, stderr: {}",
            stderr(&output)
        );
        assert!(
            stderr(&output)
                .lines()
                .any(|line| line == "pns: usage: pns doctor [--no-color]"),
            "arguments: {arguments:?}, stderr: {}",
            stderr(&output)
        );
        assert_eq!(stdout(&output), "", "arguments: {arguments:?}");
        for channel in ["mobile", "macos-banner", "hermes"] {
            assert!(
                !sandbox.fired(channel),
                "{channel} was sent a payload by a refused command: {arguments:?}"
            );
        }
        // BEFORE ANYTHING IS SENT OR PRINTED includes before anything is
        // SPAWNED. The pairing check runs another program, and a refusal that
        // still reached for it would put a network call and five seconds
        // behind a command the operator typed wrong.
        let spawned = moshi_hook_argv(&sandbox);
        assert!(
            spawned.is_empty(),
            "a refused doctor spawned moshi-hook {spawned:?}: {arguments:?}"
        );
    }
}
