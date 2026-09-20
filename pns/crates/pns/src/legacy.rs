mod argv;
mod usage;

use argv::parse_args;
pub use argv::{is_help_flag, remind_switch};
pub use usage::{SEND_USAGE, USAGE};

/// The exit code for input pns will not honour, whoever asked: a retired flag,
/// an unusable name, or a delivery class no config defines.
pub(crate) const REFUSED_INPUT: i32 = 2;

/// One notification from argv, or a usage print when `--help`/`-h` reached
/// the parse in FLAG position.
pub fn run(argv: &[String], submit: impl FnOnce(pns_domain::EventArgs, String) -> i32) -> i32 {
    let parsed = parse_args(argv.iter().cloned());
    // HELP WINS BEFORE ANYTHING ELSE ON THIS PATH: no config load, no probe.
    // It used to reach EVERYTHING when it fell through this same parser as an
    // unknown token, which notified about an empty event and raised a banner
    // titled "pns · done". Nothing about printing the commands needs the
    // machine read.
    if parsed.help {
        print!("{SEND_USAGE}");
        return 0;
    }
    for warning in &parsed.warnings {
        eprintln!("pns: {warning}");
    }
    // READ BEFORE `into_event` CONSUMES IT, and applied after the delivery: it
    // decides whether the caller hears the answer, never whether the answer is
    // computed.
    let require_delivery = parsed.require_delivery;
    let session = parsed.session.clone();
    let event = match parsed.into_event() {
        Ok(Some(event)) => event,
        Ok(None) => return 0,
        Err(error) => {
            eprintln!("pns: {error}");
            return REFUSED_INPUT;
        }
    };
    let code = submit(event, session);
    // DECISION 0010: a notification never fails the work it reports on. A
    // caller that did not ask gets 0 whatever the gateway answered, which is
    // what every harness hook, the shell notifier and the daemon rely on.
    //
    // REFUSED INPUT IS THE EXCEPTION, on the same terms as the refusals above
    // it: a gateway that would not take the page is pns's problem to report
    // quietly, while a field pns will not honour is the caller's to fix, and
    // one silently answered 0 is a page nobody learns is not being sent.
    if require_delivery || code == REFUSED_INPUT {
        code
    } else {
        0
    }
}

#[cfg(test)]
mod tests;
