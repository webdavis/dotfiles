use pns_domain::setup_answer as answered;
use std::io::{IsTerminal, Write};
mod hushed;
use hushed::Hushed;

pub struct ConsoleTerminal;
impl pns_application::Terminal for ConsoleTerminal {
    fn is_terminal(&self) -> bool {
        std::io::stdin().is_terminal()
    }
    fn say(&self, line: &str) {
        println!("{line}");
    }
    fn open(&self, invocation: &str, lines: &[(&str, &str)]) {
        let paint = crate::style::Paint::for_stdout();
        let header: Vec<crate::style::HeaderLine<'_>> = lines
            .iter()
            .map(|(label, text)| crate::style::HeaderLine { label, text })
            .collect();
        for line in crate::style::header(paint, invocation, &header) {
            println!("{line}");
        }
    }
    fn section(&self, title: &str, blurb: &str) {
        let paint = crate::style::Paint::for_stdout();
        println!();
        println!("{}", crate::style::heading(paint, title, blurb));
        println!();
    }
    fn ask(&self, question: &str) -> Result<String, String> {
        ask(question)
    }
    fn ask_hidden(&self, question: &str) -> Result<String, String> {
        ask_hidden(question)
    }
}

/// One question, and the line typed back. An `Err` names why nothing did: the
/// input ending and a read failing are different reasons, and this walk asks
/// for pasted answers, so a byte that is not valid UTF-8 is not a rare guest.
fn ask(question: &str) -> Result<String, String> {
    print!("{question}: ");
    let _ = std::io::stdout().flush();
    read_answer()
}

/// The same question, answered with the terminal's echo held off so a typed
/// secret never reaches the pane grid, herdr's persisted pane history, or any
/// attached client. THE GUARD ARMS BEFORE THE PROMPT PRINTS: arming after
/// would leave a window in which the prompt is already visible but echo is
/// still on, so an operator who types ahead of it, or this crate's own pty
/// test, could still have a secret echoed before `TCSAFLUSH` takes hold.
///
/// ONE CLIENT IS OUTSIDE THIS GUARD'S REACH: mosh, the transport under a
/// Moshi-connected phone, predicts keystrokes locally and can draw them on
/// that client transiently, ahead of the terminal's own echo state. Nothing
/// here controls that.
///
/// Ctrl-C, Ctrl-\, Ctrl-Z, a TERM or HUP, an alarm, and the two tty-stop
/// signals a backgrounded read raises are all held for the read rather than
/// answered immediately, the same trade `readpassphrase(3)` makes: each is
/// still delivered, just not until the guard drops, so Ctrl-C takes effect at
/// the next Enter rather than instantly.
fn ask_hidden(question: &str) -> Result<String, String> {
    let _hushed = Hushed::arm()?;
    print!("{question}: ");
    let _ = std::io::stdout().flush();
    read_answer()
}

/// What every read shares, hidden or not.
fn read_answer() -> Result<String, String> {
    let mut typed = String::new();
    match std::io::stdin().read_line(&mut typed) {
        Ok(0) => Err("the answers ended before the walk did".to_string()),
        Err(error) => Err(read_failure(&error, reading_from_the_background())),
        Ok(_) => Ok(answered(&typed)),
    }
}

/// Whether stdin's terminal is currently owned by some OTHER process group.
///
/// A FAILED `tcgetpgrp` IS NOT THIS CASE: a terminal that hung up answers -1
/// as well, and a read that failed on a dead terminal really did fail for its
/// own reason. A zero is no foreground group at all, which is not this either.
fn reading_from_the_background() -> bool {
    let foreground = unsafe { libc::tcgetpgrp(libc::STDIN_FILENO) };
    foreground > 0 && foreground != unsafe { libc::getpgrp() }
}

/// Why a read failed, in terms the operator can act on.
///
/// EIO FROM A BACKGROUND JOB IS NOT AN I/O FAULT, it is job control. The
/// hidden read blocks SIGTTIN, which is the set `readpassphrase(3)` holds and
/// what stops a suspension from stranding the terminal echo-off. termios(4)
/// names the trade directly: a background process that blocks or ignores
/// SIGTTIN gets `EIO` from the read "and no signal is sent", where an
/// unblocked one would have been stopped and could be resumed with `fg`.
///
/// Passed straight through, `pns setup &` therefore refuses with "Input/output
/// error", which names the symptom and hides the only thing the operator can
/// do about it. BOTH HALVES ARE REQUIRED: a bare EIO on a hung-up terminal is
/// a real failure, and a non-EIO error from the background (a non-UTF-8 paste,
/// say) still has its own honest reason to give.
fn read_failure(error: &std::io::Error, in_background: bool) -> String {
    if in_background && error.raw_os_error() == Some(libc::EIO) {
        return "this walk cannot read the terminal from the background; \
                bring it to the foreground with fg"
            .to_string();
    }
    format!("the answers could not be read: {error}")
}

#[cfg(test)]
#[path = "terminal/tests.rs"]
mod setup_walk_tests;
