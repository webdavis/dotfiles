use super::*;

// --- the decision log -------------------------------------------------------

/// An event with its state directory inside the sandbox, which is where the
/// decision ring lands.
///
/// `PNS_STATE_DIR` RIDES ON THE COMMAND, never through `set_var`: this binary
/// is threaded, and a process-wide mutation would decide another test's ring.
pub(super) fn logged_event(sandbox: &Sandbox) -> std::process::Command {
    let mut command = sandbox.pns();
    command.env("PNS_STATE_DIR", sandbox.path("state"));
    command
}

pub(super) fn acknowledged_banner(sandbox: &Sandbox) -> std::process::Command {
    let mut command = logged_event(sandbox);
    command
        .env_remove("PNS_CHANNELS_DIR")
        .env("PNS_IDLE_SECS", "0")
        .arg("--local-only");
    sandbox.stub_notifier(&mut command);
    sandbox.stub_herdr(&mut command, false);
    command
}

/// The ring, oldest first, which is the order an append leaves it in.
pub(super) fn decisions(sandbox: &Sandbox) -> Vec<String> {
    stored_records::lines(sandbox, "decisions")
}

/// The ring's path, with the state directory that holds it already made, so a
/// test can plant something hostile there before the first event runs.
pub(super) fn ring_path(sandbox: &Sandbox) -> std::path::PathBuf {
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    sandbox.path("state/decisions")
}

/// A real FIFO at that path, which is the fixture every parking test needs:
/// opening one BLOCKS until the other end is opened, for reading as well as
/// for writing.
pub(super) fn plant_fifo(path: &std::path::Path) {
    assert!(
        std::process::Command::new("mkfifo")
            .arg(path)
            .status()
            .expect("mkfifo runs")
            .success(),
        "the fixture has to be a real FIFO"
    );
}

/// One event, run to completion under a WALL-CLOCK DEADLINE.
///
/// `run` waits forever, which is the wrong instrument for a path whose bug is
/// that it PARKS: a regression would hang the whole suite with no failure to
/// read. The deadline is loose enough that a loaded machine cannot trip it,
/// and the child is killed before the panic so nothing is left holding the
/// fixture open.
pub(super) fn run_before_the_deadline(
    command: &mut std::process::Command,
) -> std::process::ExitStatus {
    output_before_the_deadline(command).status
}

/// The same wait, keeping what the run said. Piped rather than discarded
/// because a caller also has to prove the event stayed SILENT about the file
/// it could not write, and because the doctor's whole report is read back off
/// it; the volume is a handful of lines, far inside a pipe buffer, so the poll
/// below cannot deadlock on a full one.
pub(super) fn output_before_the_deadline(
    command: &mut std::process::Command,
) -> std::process::Output {
    let mut child = command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("the engine starts");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    loop {
        if child.try_wait().expect("the child is waitable").is_some() {
            return child.wait_with_output().expect("the child is waitable");
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("it never returned: it parked on a state file's path");
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// The section's heading, whose second half is where the actionId is told
/// honestly rather than printed as an empty field.
pub(super) const DECISION_HEADING_TAIL: &str = " newest first (why a card did or did not fire). No actionId \
     is recorded: moshi mints it inside the approval round trip and never hands it back.";

/// What an absent ring says, parenthesis included.
pub(super) const NO_DECISION_RECORDED: &str = "pns doctor: no decision has been recorded yet \
     (no event has run since this was installed, or none could be written).";

// --- the missed-notification journal ----------------------------------------

/// The journal's own depth, stated here rather than imported: a test that read
/// the constant it is checking would agree with any value the source held.
pub(super) const JOURNAL_KEPT: usize = 25;

/// The decision ring's depth, for the same reason.
pub(super) const RING_KEPT: usize = 5;

/// The operator's mute, published straight into the state directory.
///
/// THE ONLY WAY AN EVENT IS MISSED IN A TEST, and not a shortcut: the mute is
/// the one thing that zeroes a plan the matrix would have decorated, which is
/// what the journal exists to queue. Written rather than spawned through
/// `pns quiet`, because the engine reads one absolute expiry and a test can
/// state one without a second process.
pub(super) fn mute(sandbox: &Sandbox) {
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    let expiry = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("a clock past 1970")
        .as_secs()
        + 600;
    if sandbox.path("state/pns.db").exists() {
        pns_adapters::SqliteStore::for_records(sandbox.path("state"))
            .set_quiet_expiry(Some(expiry))
            .expect("the mute");
    } else {
        std::fs::write(sandbox.path("state/quiet-until"), format!("{expiry}\n"))
            .expect("the legacy mute before first import");
    }
}

/// The journal's path, with the state directory that holds it already made, so
/// a test can plant something there before the first event runs.
pub(super) fn journal_path(sandbox: &Sandbox) -> std::path::PathBuf {
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    sandbox.path("state/missed-notifications")
}

/// The journal, oldest first, which is the order an append leaves it in.
pub(super) fn journal(sandbox: &Sandbox) -> Vec<String> {
    stored_records::lines(sandbox, "journal")
}

/// A journal of `count` entries, each carrying its own index, written the way
/// the engine leaves them. THE TEST IS THE REPLAYER'S STAND-IN here: reading
/// an entry back is what the file is for, and it is a test doing it rather
/// than a pns command.
pub(super) fn planted_journal(count: usize) -> String {
    (0..count)
        .map(|which| {
            format!("{{\"at\":1756499000,\"agent\":\"claude\",\"state\":\"done\",\"project\":\"p\",\"branch\":\"b\",\"detail\":\"planted {which}\"}}\n")
        })
        .collect()
}

/// One entry's field, parsed. Only a test reads these.
pub(super) fn field(entry: &str, name: &str) -> String {
    let parsed: serde_json::Value =
        serde_json::from_str(entry).unwrap_or_else(|error| panic!("{error}: {entry}"));
    parsed[name].as_str().unwrap_or_default().to_string()
}

/// The journal's permission bits.
pub(super) fn journal_mode(sandbox: &Sandbox) -> u32 {
    std::os::unix::fs::PermissionsExt::mode(
        &std::fs::metadata(sandbox.path("state/pns.db"))
            .expect("the journal")
            .permissions(),
    ) & 0o777
}

/// What the doctor says about a journal holding two entries. The sentence
/// names the replayer now, because the binary has one.
pub(super) const TWO_WAITING: &str = "pns doctor: 2 missed notifications are waiting to be replayed; \
     the next event that raises a banner or a card while the operator is not away \
     delivers them.";

/// What it says when there is none, which is deliberately about what is
/// RECORDED: an empty journal means either nothing was missed or a write did
/// not land, and the line claims neither.
pub(super) const NONE_WAITING: &str = "pns doctor: no missed notification is recorded.";
