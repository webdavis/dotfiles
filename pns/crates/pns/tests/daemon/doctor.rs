use super::*;

/// THE DOCTOR'S EXIT CODE DOES NOT MOVE, in the state where it would be most
/// tempting to move it.
///
/// ADDED BEYOND THE BRIEF'S FIFTEEN, which asks for exactly this if no
/// assertion already covers it: `exit_code` cannot see the daemon at all, so
/// the only place the mistake could be made is the composition root, and only
/// a run of the real binary reaches that.
#[test]
fn the_doctor_reports_a_dead_daemon_without_moving_its_exit_code() {
    let sandbox = Sandbox::new("daemon-doctor-line");
    sandbox.write_config("[plugins.macos-banner]\nenabled = true\n");
    let mut command = sandbox.pns_stateful();
    command.env("MOSHI_HOOK_BIN", sandbox.path("no-moshi-hook-here"));
    let output = run(command.arg("doctor"));
    assert!(
        stdout(&output).contains("the daemon is enabled and has not run yet"),
        "the doctor must report the clock: {}",
        stdout(&output)
    );

    // And with a beat too old to vouch for, it still says so and still exits 0.
    let state = sandbox.state();
    std::fs::create_dir_all(&state).expect("the state directory");
    std::fs::write(
        state.join("daemon-heartbeat"),
        format!("4321 {}\n", now_secs() - 3_600),
    )
    .expect("a stale heartbeat");
    let mut command = sandbox.pns_stateful();
    command.env("MOSHI_HOOK_BIN", sandbox.path("no-moshi-hook-here"));
    let output = run(command.arg("doctor"));
    assert!(
        stdout(&output).contains("so it is not running"),
        "a stale beat must read as not running: {}",
        stdout(&output)
    );
}

/// A FIFO WHERE THE HEARTBEAT SHOULD BE MUST NOT HANG THE DOCTOR.
///
/// `open` on a named pipe blocks until a writer arrives, so a doctor that read
/// whatever it found there would never reach its own exit code, its pairing
/// check, or any of the lines below this one. Bounded here, so the regression
/// reads as a failed assertion rather than as a run that never ends.
#[test]
fn a_heartbeat_that_is_not_a_regular_file_is_refused_rather_than_opened() {
    let sandbox = Sandbox::new("daemon-doctor-refuses-a-fifo");
    sandbox.write_config(
        "[plugins.macos-banner]
enabled = true
",
    );
    let state = sandbox.state();
    std::fs::create_dir_all(&state).expect("the state directory");
    assert!(
        Command::new("/usr/bin/mkfifo")
            .arg(state.join("daemon-heartbeat"))
            .status()
            .is_ok_and(|status| status.success()),
        "the test needs a real FIFO"
    );

    let log = sandbox.path("doctor.out");
    let out = std::fs::File::create(&log).expect("the doctor log");
    let errors = out.try_clone().expect("the doctor log again");
    let mut child = sandbox
        .pns_stateful()
        .arg("doctor")
        .stdin(std::process::Stdio::null())
        .stdout(out)
        .stderr(errors)
        .spawn()
        .expect("the engine runs");
    let finished = poll_until(|| child.try_wait().ok().flatten());
    if finished.is_none() {
        let _ = child.kill();
        let _ = child.wait();
        panic!("the doctor opened the FIFO and hung instead of reporting a state");
    }
    let said = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(
        said.contains("the daemon is enabled and has not run yet"),
        "a heartbeat that is not a file reads as no heartbeat: {said}"
    );
}
