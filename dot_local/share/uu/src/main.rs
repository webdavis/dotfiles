//! The uu binary: the composition root, and the only place with a main.
//!
//! ARGUMENT PARSING AND DISPATCH, and nothing else. `cli` holds the three
//! things uu does, `state` its bookkeeping under `~/.local/state/uu`,
//! `delivery` the two outbound boundaries, `runner` and `watchdog` the one
//! place a lane subject is spawned, and `system` the questions only the
//! running machine can answer.
//!
//! EXIT CODES SAY WHO FAILED. A lane that failed does not fail the run (0),
//! because the record is what reports it and the next attempt is a week away.
//! A config uu could not read, or a lane the operator asked for and did not
//! get, is uu failing (1). An argument uu does not serve is usage (2).

use unattended_upgrades::config::LANE_TYPES;

mod cli;
mod delivery;
mod runner;
mod state;
mod system;
mod watchdog;

fn main() {
    // Die on a closed pipe the way every other unix tool does. Rust ignores
    // SIGPIPE and turns the failed write into a panic instead, so `uu doctor |
    // head` would print a backtrace over the output it just produced.
    // SAFETY: restoring a signal's default disposition, before any thread or
    // handler of this program's own exists.
    unsafe { libc::signal(libc::SIGPIPE, libc::SIG_DFL) };
    std::process::exit(dispatch());
}

/// The whole CLI. A slice match rather than a chain, so an extra word is an
/// error instead of an argument nothing reads.
fn dispatch() -> i32 {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let words: Vec<&str> = argv.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["run"] => cli::run::run_mode(None),
        ["run", lane] => cli::run::run_mode(Some(lane)),
        ["doctor"] => cli::doctor::doctor_mode(),
        ["schedule", "render"] => cli::schedule::schedule_mode(),
        [] => usage("no command given"),
        [command, ..] => usage(&format!("unknown command `{command}`")),
    }
}

/// What uu serves, printed on stderr with a non-zero exit. Never a silent
/// fallthrough to help with exit 0.
fn usage(problem: &str) -> i32 {
    eprintln!(
        "uu: {problem}\n\
         usage:\n  \
           uu run [<lane>]     run every enabled lane, or just one\n  \
           uu doctor           what this config turns on, and what it cannot reach\n  \
           uu schedule render  the launchd job for the configured day and time\n\
         lane types: {}",
        LANE_TYPES.join(", ")
    );
    2
}
