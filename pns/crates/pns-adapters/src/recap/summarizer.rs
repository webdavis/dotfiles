use crate::finish_bounded;
use pns_application::Summarizer;
use pns_domain::recap::summarizer::Invocation;
use std::{process::Command, time::Duration};

pub struct ProcessSummarizer;
impl Summarizer for ProcessSummarizer {
    fn summarize(
        &self,
        invocation: &Invocation,
        deadline: Duration,
        prompt: &str,
    ) -> Option<Vec<String>> {
        pns_domain::recap::prompt::answer(&run_summarizer(invocation, deadline, prompt).ok()?)
    }
}

/// What the configured summarizer said, or WHICH WAY it did not say it.
///
/// ARGV STRAIGHT TO `Command`, NEVER THROUGH A SHELL, which is what makes the
/// table safe to hold anything: the words are the words, so there is no
/// quoting rule to get wrong and nothing in the document can be read as
/// syntax.
///
/// THE PROMPT GOES WHERE THE BACKEND TAKES IT. Three of the four known
/// harnesses read it on stdin and hermes takes it as the value of `-q`,
/// which `Invocation` states and this only obeys.
///
/// AND A SPENT BUDGET STARTS NO PROCESS AT ALL. Spawning a model only to kill
/// it on a zero-length window is a load nobody reads, and the failure line is
/// already the answer.
pub fn run_summarizer(
    invocation: &Invocation,
    deadline: Duration,
    prompt: &str,
) -> Result<String, Failure> {
    if deadline.is_zero() {
        return Err(Failure::Silent);
    }
    let mut argv = invocation.argv.clone();
    if invocation.prompt_in_argv {
        argv.push(prompt.to_string());
    }
    let (program, arguments) = argv.split_first().ok_or(Failure::Silent)?;
    let mut command = Command::new(program);
    command.args(arguments);
    // A HOME THAT CANNOT BE MADE is a backend that could not be started.
    if invocation.stripped_codex_home {
        crate::codex::isolate(&mut command).ok_or(Failure::Unstarted)?;
    }
    command
        .stdin(match invocation.prompt_in_argv {
            true => std::process::Stdio::null(),
            false => std::process::Stdio::piped(),
        })
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    let stdin_text = (!invocation.prompt_in_argv).then_some(prompt);
    // THE GROUP WATCHDOG IS THE BOUND, not a wait this thread does: a cleanup
    // child owns the process group and kills it at the deadline, so a wedged
    // backend costs this call its window and nothing else.
    let answered = finish_bounded(
        &mut command,
        stdin_text,
        std::time::Instant::now() + deadline,
        pns_domain::recap::prompt::MAX_ANSWER_BYTES as u64 + 1,
    );
    match answered {
        // A BACKEND THAT IS NOT INSTALLED IS NOT A SPECIAL CASE: the spawn
        // fails and the recap says the summarizer could not be started.
        Err(_) => Err(Failure::Unstarted),
        Ok(None) => Err(Failure::Unanswered),
        Ok(Some(text)) if text.trim().is_empty() => Err(Failure::Silent),
        Ok(Some(text)) => Ok(text),
    }
}

/// The three ways a summarizer does not answer, each its own sentence on the
/// page.
///
/// THREE AND NOT ONE, because the operator's next move differs: a backend that
/// will not start is a path or an install, one that ran and said nothing is a
/// model or a prompt, and a deadline is a number they can raise. Which of the
/// last two a non-zero exit belongs to is not knowable here, and both read as
/// "it ran and gave nothing back".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    /// The command could not be spawned at all.
    Unstarted,
    /// It ran and produced no usable answer: a non-zero exit, a deadline, or
    /// an answer past the byte ceiling.
    Unanswered,
    /// It answered with nothing but blank space, or there was no budget left
    /// to ask it.
    Silent,
}

impl Failure {
    /// The one visible line that stands in the summary's place, naming which
    /// summarizer and which failure. IT IS NOT GATED ON `-v`: a summary that
    /// silently vanished reads as a window with nothing worth saying.
    pub fn line(self, summarizer: &str, deadline: Duration) -> String {
        match self {
            Failure::Unstarted => {
                format!("the {summarizer} summarizer could not be started")
            }
            Failure::Unanswered => format!(
                "the {summarizer} summarizer gave no answer within {}",
                pns_domain::remind::waited(deadline.as_secs().max(1))
            ),
            Failure::Silent => format!("the {summarizer} summarizer answered nothing"),
        }
    }
}

#[cfg(test)]
#[path = "summarizer/tests.rs"]
mod tests;
