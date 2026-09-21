//! One `[recap.sources]` command, run over a window.
//!
//! ARGV STRAIGHT TO `Command`, NEVER THROUGH A SHELL, which is what makes the
//! keys safe to hold anything: the words are the words, so there is no quoting
//! rule to get wrong and nothing a placeholder expands to can be read as
//! syntax by anything.
//!
//! ONE ROW PER LINE, which is the whole contract. pns does not parse what the
//! command prints; it counts lines, caps each one and prints them, so a
//! command reports whatever its own tool knows.

use crate::run_bounded_reporting;
use pns_application::SourceCommands;
use pns_domain::recap::external::{Sourcing, printed};
use std::{process::Command, time::Duration};

pub struct ProcessSourceCommands;
impl SourceCommands for ProcessSourceCommands {
    fn run(&self, argv: &[String], since: Option<u64>, until: Option<u64>) -> Sourcing {
        run_source(argv, since, until, crate::local_timestamp)
    }
}

/// The command's rows, or the state saying why there are none.
///
/// A WINDOW THE CLOCK CANNOT STATE IS AN UNAVAILABLE SECTION rather than a
/// command run with the placeholder still in it: `{since}` reaching a program
/// verbatim is a search for a literal brace, and an empty window is a search
/// for everything. Both report a window nobody asked for.
///
/// A COMMAND WITH NO WINDOW TO GIVE IT keeps its words unchanged, which is how
/// `open` takes the two unbounded listings. A command that uses a placeholder
/// is then asked for a literal one, so a command meant for both forms is
/// written without them.
pub(super) fn run_source(
    argv: &[String],
    since: Option<u64>,
    until: Option<u64>,
    stamp: impl Fn(u64) -> Option<String>,
) -> Sourcing {
    let Some((program, arguments)) = argv.split_first() else {
        return Sourcing::Unconfigured;
    };
    let substituted = match (since, until) {
        (Some(since), Some(until)) => {
            let (Some(since), Some(until)) = (stamp(since), stamp(until)) else {
                return Sourcing::Unavailable;
            };
            arguments
                .iter()
                .map(|word| word.replace(SINCE, &since).replace(UNTIL, &until))
                .collect()
        }
        _ => arguments.to_vec(),
    };
    let mut command = Command::new(program);
    command.args(&substituted);
    match run_bounded_reporting(command, DEADLINE, READ_MAX) {
        Ok(output) => Sourcing::Read(
            output
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(printed)
                .collect(),
            false,
        ),
        Err(Some(code)) => Sourcing::Failed(code),
        Err(None) => Sourcing::Unavailable,
    }
}

/// The two words a window is substituted into, spelled the way the design
/// states them.
const SINCE: &str = "{since}";
const UNTIL: &str = "{until}";

/// How long one source command may take.
///
/// THIRTY SECONDS, the same bound the GitHub listing took: far past the second
/// a local tool spends, short of anything a person would call working, and
/// nobody is waiting on it. It exists to stop a wedged network call holding
/// the whole recap rather than to hurry a slow one.
const DEADLINE: Duration = Duration::from_secs(30);

/// How much of one command's output is read. A row is a line, so this is a
/// bound on a program that decided to print a repository instead.
const READ_MAX: u64 = 512 * 1024;

#[cfg(test)]
#[path = "source_command/tests.rs"]
mod tests;
