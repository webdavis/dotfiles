//! `posture jobs`: the schedule posture installs for itself.
//!
//! WHY THE PRODUCT OWNS THIS. Every posture subcommand samples current state
//! and exits, so a schedule outside the binary is what makes the pipeline run
//! at all. A machine whose installer ships its own launchd units has one;
//! anyone who installed posture from its own source has nothing, and a
//! security tool that never runs reports all clear forever.
//!
//! FOUR VERBS, AND ONLY ONE OF THEM WRITES. `install` writes each unit and
//! loads it, `verify` asserts each one exists, is loaded and matches what
//! posture would write, `list` puts what posture expects beside the live
//! state, and `print` dumps one unit to standard output. `verify` STOPS AT
//! INSTALLED AND LOADED: whether a job ran and exited zero is
//! `posture watchdog`'s question, and two answers to it would disagree.

use posture_domain::Agent;
use std::{ffi::OsString, io::Write};

mod native;
mod report;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Verb {
    Install,
    Verify,
    List,
    Print(Agent),
}

const USAGE: &str = "usage: posture jobs install|verify|list|print <job>\n  \
                     jobs: watchdog alert poll funnel digest heartbeat\n";

pub(super) fn run(args: &[OsString], stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    match decode(args) {
        Some(verb) => native::run(verb, stdout, stderr),
        None => {
            let _ = stderr.write_all(USAGE.as_bytes());
            2
        }
    }
}

/// The one verb these words name, or nothing. An unknown word, a missing job
/// and a trailing operand are all nothing: a silent fallthrough would install
/// or skip whatever the reader assumed.
fn decode(args: &[OsString]) -> Option<Verb> {
    let words: Vec<&str> = args.iter().filter_map(|word| word.to_str()).collect();
    match (words.len() == args.len()).then_some(words.as_slice())? {
        ["install"] => Some(Verb::Install),
        ["verify"] => Some(Verb::Verify),
        ["list"] => Some(Verb::List),
        ["print", job] => Agent::ALL
            .iter()
            .find(|agent| agent.key() == *job)
            .map(|agent| Verb::Print(*agent)),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
