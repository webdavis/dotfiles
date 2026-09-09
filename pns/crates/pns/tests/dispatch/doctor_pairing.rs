use super::*;

// --- the moshi pairing check ------------------------------------------------

#[test]
fn the_doctor_prints_the_pairing_section_between_its_summary_and_the_decision_section() {
    // HEALTH SITS WITH HEALTH AND HISTORY GOES LAST. The pairing check can
    // move the exit code and the decision log explicitly cannot, so grouping
    // them the other way would put a gradeable line below an ungradeable one.
    let sandbox = Sandbox::new("doctor-pairing-placement");
    sandbox.write_config(EVERY_DISPATCHED_CHANNEL);
    run(logged_event(&sandbox).args(["--agent", "claude", "--state", "done"]));
    let mut command = doctor_command(&sandbox);
    stub_moshi_hook(
        &sandbox,
        &mut command,
        PAIRED_STATUS_JSON,
        PAIRED_STATUS_PLAIN,
    );
    let output = command.output().expect("the engine runs");

    let printed = stdout(&output);
    let lines = report_rows(&printed);
    let summary = lines
        .iter()
        .position(|line| *line == "pns doctor: 3 sent, 0 failed, 3 skipped")
        .unwrap_or_else(|| panic!("no summary line in {printed}"));
    assert_eq!(lines[summary + 1], PAIRED_LINE, "{printed}");
    assert_eq!(lines[summary + 2], MOSHI_SAYS_LINE, "{printed}");
    // GATE STATE BETWEEN THE TWO, which is the same rule one rung down: a
    // Focus being on is not a fault, so it sits below the check that can move
    // the exit code and above the history it explains.
    assert_eq!(lines[summary + 3], FOCUS_OFF_LINE, "{printed}");
    // AND THE CLOCK BESIDE IT, for the same reason and under the same rule: a
    // daemon that is down is not a fault either, so it reports here rather
    // than moving the exit code.
    assert_eq!(lines[summary + 4], DAEMON_NEVER_RAN_LINE, "{printed}");
    // AND THE NAG IMMEDIATELY UNDER THE CLOCK, which is the placement that
    // carries the one fact its own sentence leaves out: a nag with a dead daemon
    // never fires, and the line above already says whether the daemon is up.
    assert_eq!(lines[summary + 5], NAG_OFF_LINE, "{printed}");
    assert_eq!(lines[summary + 6], LIGHTS_OFF_LINE, "{printed}");
    assert_eq!(
        lines[summary + 7],
        // Two legs are pending, so the summary names the detail view. That
        // pointer is what the count is FOR: a reader holding a number and no
        // next step is where this line used to leave them.
        "pns doctor: delivery ledger: 2 pending leg(s), 0 deadlettered, growth streak 0, \
         alarm acknowledged; recording gaps none in recent daemon log; \
         run `pns failures` for what is not arriving",
        "delivery health precedes decision history: {printed}"
    );
    // The routes sit IMMEDIATELY UNDER the ledger, because the two answer one
    // question between them: what is not arriving, and whether the gateway
    // would take it if pns sent it again. This fixture has posted to no route,
    // so the section is its own summary alone.
    assert_eq!(
        lines[summary + 8],
        "pns doctor: no routes to check; nothing has been posted yet",
        "the route check sits under the ledger: {printed}"
    );
    assert_eq!(
        lines[summary + 9],
        format!("pns doctor: the last decision,{DECISION_HEADING_TAIL}"),
        "the decision section still comes last: {printed}"
    );
}

#[test]
fn the_doctor_runs_moshi_hook_exactly_twice_and_never_probes() {
    // TWO SPAWNS OF ONE SUBCOMMAND, and `probe` ZERO TIMES. Measured on 0.3.3,
    // probe answers `running: true` and `gateway: true` against a HOME holding
    // no pairing at all, so nothing it reports can be stated honestly.
    let sandbox = Sandbox::new("doctor-pairing-argv");
    sandbox.write_config(EVERY_DISPATCHED_CHANNEL);
    let mut command = doctor_command(&sandbox);
    stub_moshi_hook(
        &sandbox,
        &mut command,
        PAIRED_STATUS_JSON,
        PAIRED_STATUS_PLAIN,
    );
    command.output().expect("the engine runs");

    let recorded = moshi_hook_argv(&sandbox);
    assert_eq!(
        recorded,
        [vec!["status", "--json"], vec!["status"]],
        "the local fact is read first and off its own call, so a slow network \
         cannot cost the doctor an answer it already had"
    );
    assert!(
        !recorded
            .iter()
            .flatten()
            .any(|argument| argument == "probe"),
        "{recorded:?}"
    );
}

#[test]
fn a_doctor_with_no_moshi_hook_to_run_says_so_and_leaves_the_exit_code_to_the_sends() {
    // A MACHINE THAT DOES NOT USE MOSHI MUST NOT FAIL ITS DOCTOR FOREVER. The
    // helper already points this at a path that does not exist, which is the
    // real absent case rather than a flag.
    let sandbox = Sandbox::new("doctor-pairing-absent");
    sandbox.write_config(EVERY_DISPATCHED_CHANNEL);
    let output = doctor_command(&sandbox).output().expect("the engine runs");

    let printed = stdout(&output);
    assert!(printed.contains(NO_MOSHI_HOOK_LINE), "{printed}");
    assert!(
        !printed.contains("moshi says"),
        "there is nothing to relay: {printed}"
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "the sends alone earned it: {}",
        stderr(&output)
    );
}

#[test]
fn a_moshi_hook_that_never_returns_does_not_park_the_doctor() {
    // THE PLAIN CALL IS THE ONLY NETWORK I/O THE DOCTOR DOES ON ITS OWN
    // BEHALF, so it is the one place a hang could park a hand-typed command.
    // The json call still answers, which is the whole argument for splitting
    // them: the local fact is not hostage to the network.
    let sandbox = Sandbox::new("doctor-pairing-hang");
    sandbox.write_config(EVERY_DISPATCHED_CHANNEL);
    let bin = sandbox.path("bin");
    std::fs::create_dir_all(&bin).expect("stub bin");
    let script = bin.join("moshi-hook");
    write_script(
        &script,
        &format!(
            "case \"$*\" in\n\
             *--json*) printf '%s' '{PAIRED_STATUS_JSON}' ;;\n\
             *) exec sleep 30 ;;\n\
             esac"
        ),
    );
    let mut command = doctor_command(&sandbox);
    command.env("MOSHI_HOOK_BIN", &script);
    command.env("PNS_MOSHI_STATUS_DEADLINE_MS", "200");

    let started = std::time::Instant::now();
    let output = command.output().expect("the engine runs");
    let waited = started.elapsed();

    assert!(
        waited < std::time::Duration::from_secs(2),
        "the doctor waited {waited:?} on a call it bounds"
    );
    let printed = stdout(&output);
    assert!(
        printed.contains(PAIRED_LINE),
        "the local fact answered anyway: {printed}"
    );
    assert!(
        !printed.contains("moshi says"),
        "a call that never answered relays nothing: {printed}"
    );
    assert!(
        printed.contains("pns doctor: 3 sent, 0 failed, 3 skipped"),
        "and the sections printed before it survived: {printed}"
    );
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));

    // AND THE OTHER LEG THE OTHER WAY. The json call is the one nothing was
    // pinning: it reaches no network today, but "today" is the whole reason
    // to pin it, and an unbounded spawn there parks the same hand-typed
    // command the plain leg is bounded to protect.
    let sandbox = Sandbox::new("doctor-pairing-hang-json");
    sandbox.write_config(EVERY_DISPATCHED_CHANNEL);
    let bin = sandbox.path("bin");
    std::fs::create_dir_all(&bin).expect("stub bin");
    let script = bin.join("moshi-hook");
    write_script(
        &script,
        &format!(
            "case \"$*\" in\n\
             *--json*) exec sleep 30 ;;\n\
             *) printf '%s' '{PAIRED_STATUS_PLAIN}' ;;\n\
             esac"
        ),
    );
    let mut command = doctor_command(&sandbox);
    command.env("MOSHI_HOOK_BIN", &script);
    command.env("PNS_MOSHI_JSON_DEADLINE_MS", "200");

    let started = std::time::Instant::now();
    let output = command.output().expect("the engine runs");
    let waited = started.elapsed();

    assert!(
        waited < std::time::Duration::from_secs(2),
        "the doctor waited {waited:?} on the json call"
    );
    let printed = stdout(&output);
    assert!(
        printed.contains(NO_MOSHI_HOOK_LINE),
        "a leg that never answered is a leg that could not be graded: {printed}"
    );
    assert!(
        printed.contains(MOSHI_SAYS_LINE),
        "and the other leg's answer still arrives: {printed}"
    );
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
}

#[test]
fn an_unpaired_host_exits_one_while_the_summary_still_reads_zero_failed() {
    // THE GAP THIS CHECK EXISTS FOR. Every send is green, the census reports
    // the moshi channel green over its webhook, and every approval card is
    // dead. The summary counts SENDS and says so; the pairing line is printed
    // directly above in plain words.
    let sandbox = Sandbox::new("doctor-pairing-unpaired");
    sandbox.write_config(EVERY_DISPATCHED_CHANNEL);
    let mut command = doctor_command(&sandbox);
    stub_moshi_hook(
        &sandbox,
        &mut command,
        UNPAIRED_STATUS_JSON,
        UNPAIRED_STATUS_PLAIN,
    );
    let output = command.output().expect("the engine runs");

    let printed = stdout(&output);
    assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr(&output));
    assert!(
        printed.contains("pns doctor: 3 sent, 0 failed, 3 skipped"),
        "{printed}"
    );
    assert!(
        printed.contains(
            "pns doctor: moshi pairing: this host is NOT paired, so every \
             approval card is dead until `moshi-hook pair` runs."
        ),
        "{printed}"
    );
    assert!(
        !printed.contains("moshi says"),
        "an unpaired host prints no server line at all: {printed}"
    );
}

#[test]
fn the_pairing_check_records_nothing_of_its_own() {
    // NOTHING IS WRITTEN TO THE STATE DIRECTORY. The check reads two answers
    // out of another binary and prints; a run that left a record behind would
    // be a second writer of a ring with no reader of its own.
    let sandbox = Sandbox::new("doctor-pairing-readonly");
    sandbox.write_config(EVERY_DISPATCHED_CHANNEL);
    run(logged_event(&sandbox).args(["--agent", "claude", "--state", "done"]));
    let listing = |sandbox: &Sandbox| {
        let mut names: Vec<String> = std::fs::read_dir(sandbox.path("state"))
            .expect("the state dir")
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    };
    let before = listing(&sandbox);
    let ring_before = stored_records::text(&sandbox, "decisions");

    let mut command = doctor_command(&sandbox);
    stub_moshi_hook(
        &sandbox,
        &mut command,
        PAIRED_STATUS_JSON,
        PAIRED_STATUS_PLAIN,
    );
    let output = command.output().expect("the engine runs");

    assert!(
        stdout(&output).contains(PAIRED_LINE),
        "the check has to have RUN for this to say anything: {}",
        stdout(&output)
    );
    assert_eq!(listing(&sandbox), before, "the pairing check left a file");
    assert_eq!(
        stored_records::text(&sandbox, "decisions"),
        ring_before,
        "the pairing check wrote to the ring"
    );
}

#[test]
fn an_answer_over_the_byte_cap_is_refused_on_both_legs_rather_than_read() {
    // THE DEADLINES BOUND TIME, NOT BYTES. A moshi-hook that answered
    // endlessly would be inside its window the whole time while the JSON leg
    // handed the lot to serde and the plain leg scanned every line of it.
    //
    // BOTH ANSWERS BELOW ARE WELL FORMED and would read as a healthy paired
    // host if anything read them: only their SIZE is wrong. Junk bytes would
    // land on Unreadable through serde and prove nothing about the cap.
    let sandbox = Sandbox::new("doctor-pairing-over-cap");
    sandbox.write_config(EVERY_DISPATCHED_CHANNEL);
    let bin = sandbox.path("bin");
    std::fs::create_dir_all(&bin).expect("stub bin");
    let script = bin.join("moshi-hook");
    write_script(
        &script,
        "case \"$*\" in\n\
         *--json*) printf '{\"paired\":true,\"displayName\":\"dresden\",\
         \"hostId\":\"host_over_cap\",\"pad\":\"%1100000s\"}' '' ;;\n\
         *) printf 'server:       Moshi Pro attached (usage scope: license)\\n%1100000s\\n' '' ;;\n\
         esac",
    );
    let mut command = doctor_command(&sandbox);
    command.env("MOSHI_HOOK_BIN", &script);

    let started = std::time::Instant::now();
    let output = command.output().expect("the engine runs");
    let waited = started.elapsed();

    let printed = stdout(&output);
    assert!(
        printed
            .contains("pns doctor: moshi pairing: moshi-hook answered something this cannot read."),
        "an over-cap answer is refused before it is parsed: {printed}"
    );
    assert!(
        !printed.contains("dresden") && !printed.contains("host_over_cap"),
        "the over-cap answer was parsed anyway: {printed}"
    );
    assert!(
        !printed.contains("moshi says"),
        "an over-cap answer is refused before it is scanned: {printed}"
    );
    assert!(
        waited < std::time::Duration::from_secs(5),
        "the doctor spent {waited:?} on an answer it refused"
    );
}
