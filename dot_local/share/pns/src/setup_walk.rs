use crate::*;

/// The walk itself: one question at a time, in the order the file is written.
///
/// NONE OF THIS DECIDES ANYTHING. Every answer is carried to the composer as
/// it was typed, and a blank one is what declines a feature there. An `Err`
/// is the walk ending mid-conversation, named by its own reason, which
/// publishes nothing at all rather than composing a file out of half of one.
///
/// THE CREDENTIALS ARE ASKED INSIDE THE WALK, right after the feature they
/// arm, because a feature switched on now and credentialed later is exactly
/// the empty-value config this wizard exists to avoid.
pub(crate) fn walk() -> Result<pns::setup::Answers, String> {
    println!("{SETUP_PREAMBLE}");
    let mut answers = pns::setup::Answers {
        mobile_token: ask_hidden(
            "The phone card is on. Paste moshi's webhook secret to complete it, \
             or press enter to pair later",
        )?,
        ..Default::default()
    };

    if ask_yes("Post every event to hermes, for the durable log and the recap?")? {
        answers.hermes_key = armed_secret("hermes", "the signing key that route verifies")?;
    }
    if ask_yes("Flash hue lights green when work finishes and red when it dies?")? {
        // EACH ANSWER GATES THE NEXT QUESTION: once one comes back empty the
        // feature is already declined, and the rest would be questions whose
        // answers are thrown away.
        answers.hue_bridge = armed("the light pulse", "the hue bridge's address on the network")?;
        if !answers.hue_bridge.is_empty() {
            answers.hue_key = armed_secret("the light pulse", "an API key the bridge issued")?;
        }
        if !answers.hue_key.is_empty() {
            answers.hue_rooms = list(armed(
                "the light pulse",
                "the rooms to flash, comma separated, spelled as the bridge spells them",
            )?);
        }
    }
    if ask_yes("Read whether your phone is on the home wifi, off the router's client list?")? {
        // THE BACKEND HAS A WORKING DEFAULT and every other field here does
        // not, so this is the one question enter answers rather than declines.
        // A NAME NOTHING ANSWERS DECLINES THE PROBE, said here and not only in
        // the file: the composer writes that answer's table commented out, and
        // an operator who typed their router's brand deserves to hear why.
        match router_backend(&ask(&format!(
            "Which router backend? [{}]",
            pns::home::UNIFI_TYPE
        ))?) {
            None => println!(
                "  nothing here reads that router, so the home probe stays off; \
                 the file says how to arm it"
            ),
            Some(backend) => {
                answers.router_type = backend.to_string();
                answers.router_url = armed("the home probe", "the router's URL")?;
                if !answers.router_url.is_empty() {
                    answers.router_api_key =
                        armed_secret("the home probe", "an API key the router issued")?;
                }
                if !answers.router_api_key.is_empty() {
                    answers.router_device_hostname =
                        armed("the home probe", "the phone's hostname on that router")?;
                }
            }
        }
    }
    if ask_yes("Hold notifications back while a macOS Focus is on?")? {
        answers.focus_modes = list(armed(
            "focus silencing",
            "which Focus modes mean it, comma separated",
        )?);
    }
    answers.nag = ask_yes("Card you a second time about an approval left unanswered?")?;
    Ok(answers)
}

/// One credentialed answer, and the line that says what a blank one costs.
///
/// SAID WHEN IT HAPPENS rather than only in the file: an operator who meant to
/// arm a feature and pressed enter has one chance to notice, and the composed
/// file's own commented block is read later if at all.
fn armed(feature: &str, wanted: &str) -> Result<String, String> {
    Ok(nothing_given(feature, ask(wanted)?))
}

/// The same shape as `armed`, for a secret: read with the terminal's echo
/// held off, because this is where the token, the hermes key, the hue key
/// and the router key are all asked.
fn armed_secret(feature: &str, wanted: &str) -> Result<String, String> {
    Ok(nothing_given(feature, ask_hidden(wanted)?))
}

/// What `armed` and `armed_secret` share: the line a blank answer costs.
fn nothing_given(feature: &str, answer: String) -> String {
    if answer.is_empty() {
        println!("  nothing given, so {feature} stays off; the file says how to arm it");
    }
    answer
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

/// Turns the terminal's echo off for as long as it lives. `Drop` restores
/// both the termios state and the signal mask it holds, on every exit path
/// including EOF and an unwinding panic: this crate carries no
/// `panic = "abort"`, so Drop always runs. Arming and the restore both apply
/// `TCSAFLUSH`, which also discards whatever was already queued, so a secret
/// typed ahead of its own prompt is lost rather than read, and so is an
/// answer typed ahead of the question after it.
struct Hushed {
    original: libc::termios,
    original_mask: libc::sigset_t,
}

impl Hushed {
    /// Arm the guard. FAILS CLOSED: a termios or signal call this cannot
    /// complete is refused as loudly as a bad answer, rather than silently
    /// leaving echo on and asking for a secret anyway.
    fn arm() -> Result<Hushed, String> {
        let mut original: libc::termios = unsafe { std::mem::zeroed() };
        if unsafe { libc::tcgetattr(libc::STDIN_FILENO, &mut original) } != 0 {
            return Err(format!(
                "the terminal's settings could not be read (tcgetattr: {})",
                std::io::Error::last_os_error()
            ));
        }
        let mut blocked: libc::sigset_t = unsafe { std::mem::zeroed() };
        if unsafe { libc::sigemptyset(&mut blocked) } != 0 {
            return Err(format!(
                "the signal mask could not be built (sigemptyset: {})",
                std::io::Error::last_os_error()
            ));
        }
        // BLOCKED FOR THE READ, not disabled: each one is still delivered,
        // once the guard drops and the mask is restored. THIS IS THE WHOLE
        // SET `readpassphrase(3)` HOLDS, all nine of them, because the doc
        // comment above cites that function as the model and a quietly
        // shorter set is the model's holes without its name. SIGTTIN and
        // SIGTTOU: a read that becomes a background job would otherwise be
        // stopped by SIGTTIN with echo still off, and Drop's own
        // `tcsetattr` from a background group can raise SIGTTOU before it
        // gets the chance to restore. SIGALRM: an alarm armed before the
        // walk began would otherwise end the process mid-prompt, and a
        // process that dies before `Drop` leaves the operator's terminal
        // echo-off with no prompt in front of it. SIGPIPE is inert today
        // (the Rust runtime sets it to `SIG_IGN` before `main`, so it ends
        // nothing to begin with) and is held anyway, so this set does not
        // have to be re-argued against the manual page every time the
        // runtime's own default moves.
        for signal in [
            libc::SIGINT,
            libc::SIGQUIT,
            libc::SIGTSTP,
            libc::SIGTERM,
            libc::SIGHUP,
            libc::SIGTTIN,
            libc::SIGTTOU,
            libc::SIGALRM,
            libc::SIGPIPE,
        ] {
            if unsafe { libc::sigaddset(&mut blocked, signal) } != 0 {
                return Err(format!(
                    "the signal mask could not be built (sigaddset: {})",
                    std::io::Error::last_os_error()
                ));
            }
        }
        let mut original_mask: libc::sigset_t = unsafe { std::mem::zeroed() };
        // `pthread_sigmask` IS POSIX, NOT BSD `errno`-STYLE: it RETURNS its
        // error number directly rather than setting errno, so the result
        // itself, not `last_os_error()`, is the only honest source for one.
        let masked =
            unsafe { libc::pthread_sigmask(libc::SIG_BLOCK, &blocked, &mut original_mask) };
        if masked != 0 {
            return Err(format!(
                "signals could not be held for the read (pthread_sigmask: {})",
                std::io::Error::from_raw_os_error(masked)
            ));
        }
        let mut hushed = original;
        hushed.c_lflag &= !libc::ECHO;
        hushed.c_lflag |= libc::ECHONL;
        if unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSAFLUSH, &hushed) } != 0 {
            let error = std::io::Error::last_os_error();
            unsafe {
                libc::pthread_sigmask(libc::SIG_SETMASK, &original_mask, std::ptr::null_mut());
            }
            return Err(format!(
                "the terminal's echo could not be turned off (tcsetattr: {error})"
            ));
        }
        Ok(Hushed {
            original,
            original_mask,
        })
    }
}

impl Drop for Hushed {
    fn drop(&mut self) {
        unsafe {
            // TERMIOS FIRST, THEN THE MASK: a signal delivered between the
            // two would otherwise run with the operator's terminal still
            // echo-off. Neither call's failure is checked: a tty that hung
            // up during the read (EOF from a closed pty) makes `tcsetattr`
            // fail, and Drop must never panic over a terminal already gone.
            libc::tcsetattr(libc::STDIN_FILENO, libc::TCSAFLUSH, &self.original);
            libc::pthread_sigmask(libc::SIG_SETMASK, &self.original_mask, std::ptr::null_mut());
        }
    }
}

/// What a typed line means as an answer.
///
/// A LINE OF NOTHING BUT SPACES IS A BLANK ONE, which is the rule the whole
/// walk rests on: `compose_config` declines a feature whose credential is
/// empty, and it asks `is_empty`, so a credential of two spaces would arm a
/// plugin with two spaces and deliver nothing while reading as set up. That is
/// the exact state this wizard exists to keep off a fresh machine, and the
/// trailing newline every line carries is what makes it reachable.
fn answered(line: &str) -> String {
    line.trim().to_string()
}

/// One yes-or-no question. ENTER MEANS NO, and so does anything that is not a
/// yes: this walk arms features that deliver to a phone and to lamps, and the
/// answer nobody typed on purpose must be the one that changes nothing.
fn ask_yes(question: &str) -> Result<bool, String> {
    Ok(means_yes(&ask(&format!("{question} [y/N]"))?))
}

/// Whether an answer to a yes-or-no question was a yes.
///
/// ONLY A YES IS ONE. Enter, a word nobody meant, and a mistyped `yes` all
/// mean no, because every question this answers arms something that delivers
/// to a phone or to a lamp and takes a credential to do it.
fn means_yes(answer: &str) -> bool {
    matches!(answer.to_lowercase().as_str(), "y" | "yes")
}

/// Which compiled-in backend an answer names, or `None` for one no backend
/// answers.
///
/// THE SET IS THE CODE'S, never a list kept here: `home` is what refuses a
/// type at probe time, so a wizard restating its own copy of that set would go
/// on accepting yesterday's answer the day a second backend lands. Enter names
/// the one there is, and a spelling that differs only in case is that one too,
/// written back as the code spells it rather than as it was typed.
fn router_backend(answer: &str) -> Option<&'static str> {
    (answer.is_empty() || answer.eq_ignore_ascii_case(pns::home::UNIFI_TYPE))
        .then_some(pns::home::UNIFI_TYPE)
}

/// A comma-separated answer as the values it names, blanks dropped.
fn list(answer: String) -> Vec<String> {
    answer
        .split(',')
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}
/// What the walk says before it starts asking.
const SETUP_PREAMBLE: &str = "\
pns setup: a few questions, and a config at the end of them.
The macOS banner and the phone card are on and are not asked about. Everything
else is off unless you arm it here, and enter is no. Nothing is written until
the last answer.";

#[cfg(test)]
#[path = "setup_walk/tests.rs"]
mod setup_walk_tests;
