mod argv;
mod usage;

use argv::parse_args;
pub use argv::{is_help_flag, is_producer_argv};
pub use usage::USAGE;

/// One notification from argv, or a usage print when `--help`/`-h` reached
/// the parse in FLAG position.
pub fn run(argv: &[String], submit: impl FnOnce(pns_domain::EventArgs)) -> i32 {
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
    let event = match parsed.into_event() {
        Ok(Some(event)) => event,
        Ok(None) => return 0,
        Err(error) => {
            eprintln!("pns: {error}");
            return 2;
        }
    };
    submit(event);
    0
}
