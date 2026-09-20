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
    let session = parsed.session.clone();
    let event = match parsed.into_event() {
        Ok(Some(event)) => event,
        Ok(None) => return 0,
        Err(error) => {
            eprintln!("pns: {error}");
            return REFUSED_INPUT;
        }
    };
    // THE EXIT CODE REPORTS DELIVERY, always: 0 when every destination took
    // the page, 1 when any did not, 2 for a field pns will not honour. The
    // always-exit-0 contract lives on the harness hook paths, which return
    // their own 0 whatever this answers, and the shell notifier and the daemon
    // discard the code.
    submit(event, session)
}

#[cfg(test)]
mod tests;
