use crate::run_bounded;
use pns_application::Summarizer;
use std::{process::Command, time::Duration};

pub struct ProcessSummarizer;
impl Summarizer for ProcessSummarizer {
    fn summarize(&self, argv: &[String], deadline: Duration, prompt: &str) -> Option<Vec<String>> {
        summarize(argv, deadline, prompt)
    }
}

/// The configured command, handed the window on stdin, and what it said back
/// as timeline lines. None for every way of not answering.
///
/// ARGV STRAIGHT TO `Command`, NEVER THROUGH A SHELL, which is what makes the
/// key safe to hold anything: the words are the words, so there is no quoting
/// rule to get wrong and nothing in the window can be read as syntax.
///
/// THE SEAM IS THE ONE THE PROBES ALREADY USE. `run_bounded` writes the prompt
/// inside the deadline window, reads stdout lossily, kills the child when the
/// window closes and answers None on a non-zero exit, which is every rung of
/// this ladder but the last; `recap::answer` owns that one.
///
/// A BACKEND THAT IS NOT INSTALLED IS NOT A SPECIAL CASE. The spawn fails, the
/// seam answers None, and the recap posts the plain list saying so, which is
/// the same thing the operator sees when the model is simply slow.
///
/// AND NEITHER IS A SPENT BUDGET. An episode whose deadline is gone starts no
/// process at all: spawning one only to kill it on a zero-length window is a
/// model load nobody reads, and the plain list is already the answer.
fn summarize(argv: &[String], deadline: Duration, prompt: &str) -> Option<Vec<String>> {
    if deadline.is_zero() {
        return None;
    }
    let (program, arguments) = argv.split_first()?;
    let mut command = Command::new(program);
    command.args(arguments);
    pns_domain::recap::prompt::answer(&run_bounded(
        command,
        Some(prompt),
        deadline,
        pns_domain::recap::prompt::MAX_ANSWER_BYTES as u64 + 1,
    )?)
}
