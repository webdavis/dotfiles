use super::*;

#[test]
fn a_moshi_that_never_reads_its_stdin_cannot_hold_the_notification() {
    // The write ran on this thread, so a child that does not read blocked it
    // once the pipe buffer filled: the permission request hung BEFORE the
    // notification went out and before the wait that is meant to be the only
    // place this waits on a person.
    let sandbox = Sandbox::new("hook-blocked-deaf-moshi");
    let bin = sandbox.path("bin");
    std::fs::create_dir_all(&bin).expect("stub bin");
    write_script(&bin.join("moshi-hook"), "sleep 30");
    let mut command = sandbox.pns();
    command
        .env("PNS_IDLE_SECS", "99999")
        .env("MOSHI_HOOK_BIN", bin.join("moshi-hook"));
    let mut child = spawn_hook(command, "blocked");
    // Past the 64KB pipe buffer, which is what turns a child that does not
    // read into a writer that never returns.
    let payload = format!(r#"{{"message":"{}"}}"#, "x".repeat(200_000));
    write_payload(&mut child, payload.as_bytes());
    let deadline = std::time::Instant::now() + HANG_LIMIT;
    while !sandbox.fired("hermes") && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let notified = sandbox.fired("hermes");
    // The hook is still waiting on the "human" by design, so the test ends it.
    let _ = child.kill();
    let _ = child.wait();
    assert!(
        notified,
        "the notification must not wait on a child that never reads"
    );
}

#[test]
fn a_transcript_that_never_ends_is_not_read_at_all() {
    // /dev/zero is infinite and a FIFO blocks on open: neither is a regular
    // file, and the check happens before the open for exactly that reason.
    let sandbox = Sandbox::new("hook-transcript-devzero");
    let fifo = sandbox.path("t.fifo");
    assert!(
        std::process::Command::new("/usr/bin/mkfifo")
            .arg(&fifo)
            .status()
            .expect("mkfifo")
            .success()
    );
    for path in ["/dev/zero".to_string(), fifo.display().to_string()] {
        // ONE REREAD, not the default four extra rereads after the first
        // read: the property under test is that a non-regular transcript
        // never holds the hook open at all, not how many times the reread
        // loop retries an empty reply, so the retry count is not what this
        // pins.
        let mut command = sandbox.pns();
        command.env("PNS_REPLY_REREAD_ATTEMPTS", "1");
        let mut child = spawn_hook(command, "stop");
        let payload =
            format!(r#"{{"session_id":"s1","cwd":"/a/dotfiles","transcript_path":"{path}"}}"#);
        write_payload(&mut child, payload.as_bytes());
        assert_eq!(
            finished_within(child, HANG_LIMIT),
            Some(0),
            "transcript_path {path} must not hold the hook open"
        );
    }
}

#[test]
fn a_payload_nobody_finishes_writing_still_exits_on_the_contract() {
    // The pipe stays open with nothing in it, which used to hang before any
    // of the exit-0 contract could run.
    let sandbox = Sandbox::new("hook-payload-hang");
    let mut command = sandbox.pns();
    command.env("PNS_PAYLOAD_DEADLINE_MS", "200");
    let child = spawn_hook(command, "stop");
    assert_eq!(
        finished_within(child, HANG_LIMIT),
        Some(0),
        "no payload is no notification, and still exit 0"
    );
    assert!(!sandbox.fired("hermes"), "and nothing is sent on a guess");
}

#[test]
fn a_condenser_that_closes_stdout_and_sleeps_is_killed_at_its_deadline() {
    // The case the old bound missed entirely: stdout closes, the read
    // finishes, and the wait then blocked with no deadline on it.
    let sandbox = Sandbox::new("hook-condenser-sleeps");
    let bin = sandbox.path("bin");
    std::fs::create_dir_all(&bin).expect("bin");
    write_script(&bin.join("codex"), "cat >/dev/null; exec 1>&-; sleep 30");
    let mut command = sandbox.pns();
    command
        .env("CODEX_BIN", bin.join("codex"))
        .env("PNS_CODEX_HOME", sandbox.path("codex-home"))
        .env("PNS_CONDENSER_DEADLINE_MS", "300");
    let mut child = spawn_hook(command, "stop");
    write_payload(
        &mut child,
        br#"{"session_id":"s1","cwd":"/a/dotfiles","last_assistant_message":"a turn"}"#,
    );
    assert_eq!(finished_within(child, HANG_LIMIT), Some(0));
    assert_eq!(
        sandbox.event("hermes")["detail"],
        "a turn",
        "an expired condenser falls back to the reply"
    );
}

#[test]
fn a_condenser_that_never_reads_its_stdin_is_bounded_too() {
    // The write is inside the window now: this child never drains the pipe,
    // which used to block before the clock started.
    let sandbox = Sandbox::new("hook-condenser-deaf");
    let bin = sandbox.path("bin");
    std::fs::create_dir_all(&bin).expect("bin");
    write_script(&bin.join("codex"), "sleep 30");
    let mut command = sandbox.pns();
    command
        .env("CODEX_BIN", bin.join("codex"))
        .env("PNS_CODEX_HOME", sandbox.path("codex-home"))
        .env("PNS_CONDENSER_DEADLINE_MS", "300");
    let mut child = spawn_hook(command, "stop");
    let big = "x".repeat(200_000);
    let payload =
        format!(r#"{{"session_id":"s1","cwd":"/a/dotfiles","last_assistant_message":"{big}"}}"#);
    write_payload(&mut child, payload.as_bytes());
    assert_eq!(finished_within(child, HANG_LIMIT), Some(0));
}

#[test]
fn a_stuck_multiplexer_leaves_the_view_unreadable_rather_than_blocking() {
    // Unknown never suppresses, so a herdr that hangs costs a spare
    // notification; a herdr that hangs the HOOK costs the notification.
    let sandbox = Sandbox::new("hook-herdr-stuck");
    let bin = sandbox.path("bin");
    std::fs::create_dir_all(&bin).expect("bin");
    write_script(&bin.join("herdr"), "sleep 30");
    let mut command = sandbox.pns();
    command.env("PNS_IDLE_SECS", "0");
    let mut path = std::ffi::OsString::from(&bin);
    path.push(":");
    path.push(std::env::var_os("PATH").unwrap_or_default());
    command.env("PATH", path);
    let mut child = spawn_hook(command, "stop");
    write_payload(
        &mut child,
        br#"{"session_id":"s1","cwd":"/a/dotfiles","last_assistant_message":"x","transcript_path":""}"#,
    );
    assert_eq!(finished_within(child, HANG_LIMIT), Some(0));
}

/// A moshi-hook that registers the submission and then never answers.
///
/// THE WEDGED DAEMON, which is the case an unbounded wait cannot survive: a
/// listener that accepts the connection and never replies held the real
/// `moshi-hook claude-hook` for 90 seconds with no self-timeout, no output and
/// no error (measured 2026-08-29). The argv record is written BEFORE the
/// sleep, so a submission that happened is still countable while the child is
/// still hanging.
///
/// `exec` IS LOAD BEARING. moshi-hook is a single binary, so the process pns
/// spawns is the process holding the inherited stdout, and the bound's kill
/// reaches exactly that one. Without `exec` this stub's shell would fork the
/// sleep and leave a GRANDCHILD holding the pipe open for its full ten
/// seconds, which is a submission shape the real one does not have and which
/// no kill short of a process group could release. Measured both ways: 0.001s
/// to EOF after the kill with `exec`, 9.9s without.
fn stub_silent_moshi(sandbox: &Sandbox, command: &mut Command) {
    let bin = sandbox.path("bin");
    std::fs::create_dir_all(&bin).expect("stub bin");
    write_script(
        &bin.join("moshi-hook"),
        &format!(
            "printf '%s\\n' \"$*\" >>\"{sandbox}/moshi.argv\"; cat >/dev/null; exec sleep 10",
            sandbox = sandbox.display()
        ),
    );
    command.env("MOSHI_HOOK_BIN", bin.join("moshi-hook"));
}

/// The deadline each silent-moshi run injects, and the window the harness's
/// own read of the hook must end inside: FOUR TIMES that deadline.
///
/// FOUR TIMES IS CHOSEN AGAINST A NAMED MUTANT. A deadline hard-coded to one
/// second, ignoring the injected value, puts EOF just past a second, which the
/// hook run's 600ms bound refuses. Measured green runs of that one sit at
/// 0.180-0.198s idle and 0.217s with every core busy, so the bound keeps a
/// 2.7x margin over the worst honest run and better than 3x over a quiet
/// machine. The gate run injects 400ms rather than 150 because its stub has a
/// `/bin/sh` spawn inside the window; its bound scales with it, and it
/// measures 0.408-0.420s.
const SILENT_MOSHI_DEADLINE_MS: &str = "150";
const SILENT_MOSHI_EOF_BOUND: std::time::Duration = std::time::Duration::from_millis(600);
const GATE_SILENT_MOSHI_DEADLINE_MS: &str = "400";
const GATE_SILENT_MOSHI_EOF_BOUND: std::time::Duration = std::time::Duration::from_millis(1_600);

#[test]
fn a_moshi_that_never_answers_stops_holding_the_operators_prompt() {
    // THE DEFECT THIS BOUND EXISTS FOR, on the most safety-critical path pns
    // has. PermissionRequest runs BEFORE the prompt is drawn and is
    // deliberately not async, so the harness awaits this hook: every second
    // spent waiting on a wedged daemon is a second the operator is looking at
    // a terminal that is not showing them the question, and the only other
    // bound in the system is the harness's own ten-minute ceiling.
    //
    // THE CLOCK IS THE HARNESS'S OWN, and that is the point of this row.
    // Claude Code decides this event by reading the hook's stdout to end, the
    // submission inherited that write end, and a survivor holds it open: a
    // deadline that returns without killing the child measures 0.18s on the
    // process and 10.03s on the stream the harness is actually reading. So
    // this assertion pins the KILL as much as the deadline, and reverting the
    // kill hangs it past its bound.
    //
    // EXIT 0 IS NO OPINION. Claude Code reads no exit code on this event at
    // all (measured); a non-zero would put pns's own word into a channel that
    // carries moshi's, which the gate's direct callers do read.
    //
    // THE SLEEP IS NOT A SLOW TEST. With the bound in place this run costs the
    // injected deadline and the teardown; it is the RED run, with no bound,
    // that pays the stub's ten seconds.
    let sandbox = Sandbox::new("hook-blocked-silent-moshi");
    let mut command = sandbox.pns();
    command
        .env("PNS_IDLE_SECS", "99999")
        .env("PNS_MOSHI_SUBMIT_DEADLINE_MS", SILENT_MOSHI_DEADLINE_MS);
    stub_silent_moshi(&sandbox, &mut command);
    let mut child = spawn_hook(command, "blocked");
    let stdout = child.stdout.take().expect("stdout");
    write_payload(
        &mut child,
        br#"{"message":"may I run this","session_id":"s1","cwd":"/a/dotfiles"}"#,
    );
    let hidden_for = stdout_eof_within(stdout, HANG_LIMIT)
        .expect("the harness's own read of this hook has to end at all");
    assert!(
        hidden_for < SILENT_MOSHI_EOF_BOUND,
        "the prompt stayed hidden for {hidden_for:?}, past the {SILENT_MOSHI_EOF_BOUND:?} bound"
    );
    assert_eq!(
        finished_within(child, HANG_LIMIT),
        Some(0),
        "a daemon that never answers must not hold the permission prompt off the screen"
    );
    // THE BOUND COST THE WAIT AND NOT THE NOTIFICATION. The card is raised
    // before the wait starts, so an expiry that also lost the card would mean
    // the operator learned nothing at all about a prompt they cannot see.
    assert_eq!(
        sandbox.event("hermes")["state"],
        "blocked",
        "the blocked card still went out"
    );
    // AND THE TIMEOUT SUBMITTED NOTHING FURTHER. One prompt is one submission
    // however the wait ended: a retry after an expiry is a second card and a
    // second answer to one question.
    assert_eq!(
        submissions(&sandbox),
        ["claude-hook"],
        "one prompt, one submission, expiry included"
    );
}

#[test]
fn the_gate_is_bounded_by_the_same_clock_as_the_hook() {
    // THE SECOND CALLER, which is the whole reason the bound sits at the
    // function both of them route through rather than at the one the defect
    // was found on. `pns gate <harness>-hook` is what pi and omp reach
    // directly, with no pns hook in front of it, and it waited on exactly the
    // same unbounded `child.wait()`.
    //
    // SAME CLOCK, SAME STREAM: timed to stdout EOF for the reason its twin
    // above states, and with a longer deadline because this path spawns the
    // stub's shell inside the window.
    let sandbox = Sandbox::new("gate-silent-moshi");
    let mut command = sandbox.pns();
    command.env("PNS_IDLE_SECS", "99999").env(
        "PNS_MOSHI_SUBMIT_DEADLINE_MS",
        GATE_SILENT_MOSHI_DEADLINE_MS,
    );
    stub_silent_moshi(&sandbox, &mut command);
    let mut child = command
        .args(["gate", "claude-hook"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("the engine runs");
    let stdout = child.stdout.take().expect("stdout");
    write_payload(&mut child, b"{\"ask\":1}\n");
    let hidden_for = stdout_eof_within(stdout, HANG_LIMIT)
        .expect("the harness's own read of the gate has to end at all");
    assert!(
        hidden_for < GATE_SILENT_MOSHI_EOF_BOUND,
        "the gate held its stream for {hidden_for:?}, past the {GATE_SILENT_MOSHI_EOF_BOUND:?} bound"
    );
    assert_eq!(
        finished_within(child, HANG_LIMIT),
        Some(0),
        "the gate waits on the same clock: no opinion, and the harness prompts as usual"
    );
    assert_eq!(
        submissions(&sandbox),
        ["claude-hook"],
        "one prompt, one submission, expiry included"
    );
}
