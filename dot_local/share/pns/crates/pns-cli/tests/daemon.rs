//! The daemon driven as a process: the loop, the spool drain, the spawn and
//! the reap.
//!
//! EVERY TEST HERE RUNS THE REAL BINARY through `DaemonGuard`, which kills it
//! on every exit path including a panic, and every one of them pins the state
//! directory inside its own sandbox. The pure decisions (what a job is,
//! whether it fires, what a repeat re-arms to) are unit tested in
//! `src/daemon.rs`; nothing here re-proves one.

mod support;

use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use support::{DaemonGuard, Sandbox, poll_until, run, stdout};

/// A tick fast enough that a whole test costs a fraction of a second.
const TICK_MS: u64 = 25;

/// The floor `main.rs`'s `MIN_TICK_MS` accepts: below this the daemon
/// silently falls back to its one-SECOND production default, which would
/// make a test slower rather than faster. Used only by the two tests whose
/// cost is `SWITCH_TICKS` or `CHILD_TICKS` (both 30) ticks deep, where
/// `TICK_MS` costs 750 ms; at this floor the same wait is 300 ms.
const FAST_TICK_MS: u64 = 10;

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("a clock")
        .as_secs()
}

/// The hermes stub replaced by one that APPENDS a line per delivery, so "once"
/// and "twice" are different observations. The shared stub truncates, which
/// makes a second firing indistinguishable from the first.
fn count_fires(sandbox: &Sandbox) {
    sandbox.stub_channel(
        "hermes",
        &format!("cat >>\"{}/hermes.events\"", sandbox.display()),
    );
}

/// How many times the counting stub has been handed an event.
fn fires(sandbox: &Sandbox) -> usize {
    std::fs::read_to_string(sandbox.path("hermes.events"))
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count()
}

/// One registration through the typed command, which is the same library call
/// a rider will make.
fn schedule(sandbox: &Sandbox, flags: &[&str], args: &[&str]) -> std::process::Output {
    let mut command = sandbox.pns_stateful();
    command.args(["daemon", "schedule"]);
    command.args(flags);
    command.arg("--");
    command.args(args);
    command.output().expect("the engine runs")
}

/// The one channel a scheduled job's event reaches.
///
/// A CONFIG IS NOT OPTIONAL here: with none, the re-executed child selects no
/// plugin at all, so the daemon would report a job run and nothing would be
/// delivered. HERMES rather than the banner, because the sandbox pins the
/// operator AWAY (`PNS_IDLE_SECS` at a day), and a banner on a screen nobody is
/// sitting at is exactly what the engine declines to raise.
const ONE_CHANNEL: &str = "[plugins.hermes]\nenabled = true\nkey = \"k\"\n";

/// An ordinary event for a scheduled job to deliver.
const EVENT: [&str; 6] = [
    "--agent",
    "pns",
    "--state",
    "done",
    "--detail",
    "a scheduled job",
];

/// Whatever is left in the spool.
fn spooled(sandbox: &Sandbox) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(sandbox.state().join("daemon"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[path = "daemon/scheduling.rs"]
mod scheduling;

#[path = "daemon/lifecycle.rs"]
mod lifecycle;

#[path = "daemon/output.rs"]
mod output;

#[path = "daemon/doctor.rs"]
mod doctor;

#[path = "daemon/hooks.rs"]
mod hooks;

#[path = "daemon/spool.rs"]
mod spool;
