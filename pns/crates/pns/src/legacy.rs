mod argv;
mod usage;

use argv::parse_args;
pub use argv::{is_help_flag, is_producer_argv};
pub use usage::USAGE;

/// One notification from argv, or a usage print when `--help`/`-h` reached
/// the parse in FLAG position.
pub fn run(argv: &[String], submit: impl FnOnce(pns_domain::EventArgs) -> i32) -> i32 {
    let parsed = parse_args(argv.iter().cloned());
    // HELP WINS BEFORE ANYTHING ELSE ON THIS PATH: no config load, no probe.
    // It used to reach EVERYTHING when it fell through this same parser as an
    // unknown token, which notified about an empty event and raised a banner
    // titled "pns · done". Nothing about printing the commands needs the
    // machine read.
    if parsed.help {
        print!("{USAGE}");
        return 0;
    }
    for warning in &parsed.warnings {
        eprintln!("pns: {warning}");
    }
    // READ BEFORE `into_event` CONSUMES IT, and applied after the delivery: it
    // decides whether the caller hears the answer, never whether the answer is
    // computed.
    let require_delivery = parsed.require_delivery;
    let event = match parsed.into_event() {
        Ok(Some(event)) => event,
        Ok(None) => return 0,
        Err(argv::Refusal::Scope) => {
            println!(
                "pns: post SKIPPED, --local-only and --remote-only were both given, which suppresses every channel; nothing was sent"
            );
            return 0;
        }
        Err(argv::Refusal::Elapsed(error)) => {
            eprintln!("pns: {error}");
            return 2;
        }
    };
    let code = submit(event);
    // DECISION 0010: a notification never fails the work it reports on. A
    // caller that did not ask gets 0 whatever the gateway answered, which is
    // what every harness hook, the shell notifier and the daemon rely on.
    if require_delivery { code } else { 0 }
}

#[cfg(test)]
mod tests;
