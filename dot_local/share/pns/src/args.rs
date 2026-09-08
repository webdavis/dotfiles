//! pns's legacy CLI contract is lenient; elapsed timing is validated separately.
//!
//! Legacy arguments sit on an always-exit-0 notification path, so their
//! problems WARN and degrade rather than abort. Elapsed timing refuses invalid
//! input before notification. Four rules carry the
//! contract: a value-taking flag whose next token is missing or is itself a
//! RECOGNIZED flag is warned about and ignored WITHOUT consuming that token
//! (consuming it would silently drop the real flag, e.g. leak an event a
//! caller narrowed with `--pane --local-only`); an unrecognized next token
//! IS taken as the value, the leniency the bash deliberately retained; any
//! other unknown argument is skipped silently; and `--help`/`-h` sitting in
//! FLAG position wins over all three and turns the event into a usage print,
//! while the same word sitting where a flag's value belongs is still just a
//! value, under the second rule.

/// The parsed event arguments. THE VALUE MOVED to `pns-domain`, beside the
/// rendered `Event` it becomes, because the use cases in `pns-application`
/// take one and a use case may not name a type this package owns. The PARSE
/// below stayed, which is the half that is about a command line.
pub use pns_domain::EventArgs;

/// Every flag that takes a value. Private: the only consumers are the
/// predicates in this module. It used to be `pub` so a test could assert the
/// hand-typed usage text mentioned every flag, a declaration-parity check
/// rather than one about this parser's behavior; that test is gone.
const VALUE_FLAGS: [&str; 8] = [
    "--agent",
    "--state",
    "--project",
    "--branch",
    "--detail",
    "--pane",
    "--channel",
    "--elapsed",
];

/// Every flag that takes no value. It is a LIST rather than a chain of
/// comparisons because the chain is what went stale: `--long-running` was
/// handled below and never added here, so a value flag in front of it ate it as
/// its value and the tier vanished without a warning.
const BARE_FLAGS: [&str; 3] = ["--long-running", "--local-only", "--remote-only"];

/// Whether a token is a flag this parser recognizes.
///
/// PUBLIC BECAUSE THE COMPOSITION ROOT ASKS IT TOO: telling a producer
/// invocation from a mistyped subcommand is a question about these same two
/// lists, and a second copy of them in `main` is exactly the drift the
/// `--long-running` bug above came from.
pub fn is_producer_flag(token: &str) -> bool {
    VALUE_FLAGS.contains(&token) || BARE_FLAGS.contains(&token)
}

/// Whether a token is `--help`/`-h`.
///
/// PUBLIC FOR THE SAME REASON `is_producer_flag` IS: the composition root's
/// producer check counts it too (a producer invocation that only adds
/// `--help` still has to reach this parser, which is where the help arm
/// actually prints the usage), and a second copy of the two spellings in
/// `main` is exactly the drift the `--long-running` bug above came from.
pub fn is_help_flag(token: &str) -> bool {
    token == "--help" || token == "-h"
}

pub struct ParsedArgs {
    pub event: EventArgs,
    pub warnings: Vec<String>,
    elapsed: Result<Option<u64>, String>,
}

impl ParsedArgs {
    pub fn into_event(self) -> Result<Option<EventArgs>, String> {
        match self.elapsed? {
            Some(seconds) => Ok(pns_domain::elapsed_event(self.event, seconds)),
            None => Ok(Some(self.event)),
        }
    }
}

/// Parse argv, retaining legacy warnings and a separate elapsed refusal.
pub fn parse_args<I>(argv: I) -> ParsedArgs
where
    I: IntoIterator<Item = String>,
{
    let mut parsed = EventArgs::default();
    let mut warnings = Vec::new();
    let mut elapsed = Ok(None);
    let mut tokens = argv.into_iter().peekable();
    while let Some(token) = tokens.next() {
        match token.as_str() {
            "--long-running" => parsed.long_running = true,
            "--local-only" => parsed.local_only = true,
            "--remote-only" => parsed.remote_only = true,
            // HELP IN FLAG POSITION WINS: this arm only ever sees a token
            // that reached the top of the loop unconsumed, so `--state
            // --help` never lands here, the value arm below already took
            // `--help` as `--state`'s value by the time this token is asked
            // about again.
            flag if is_help_flag(flag) => parsed.help = true,
            "--elapsed" => {
                let value = if tokens.peek().is_some_and(|next| !is_producer_flag(next)) {
                    tokens.next()
                } else {
                    None
                };
                let seconds = value
                    .as_deref()
                    .filter(|value| {
                        !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
                    })
                    .and_then(|value| value.parse::<u64>().ok());
                if elapsed.is_ok() {
                    elapsed = seconds.map(Some).ok_or_else(|| {
                        "--elapsed requires a nonnegative whole number of seconds".to_owned()
                    });
                }
            }
            flag if VALUE_FLAGS.contains(&flag) => {
                // Missing, or a recognized flag standing where the value
                // should be: warn and leave the token for its own arm.
                if tokens.peek().is_none_or(|next| is_producer_flag(next)) {
                    warnings.push(format!("{flag} given without a value; ignoring"));
                    continue;
                }
                let Some(value) = tokens.next() else { continue };
                match flag {
                    "--agent" => parsed.agent = value,
                    "--state" => parsed.state = value,
                    "--project" => parsed.project = value,
                    "--branch" => parsed.branch = value,
                    "--detail" => parsed.detail = value,
                    "--channel" => parsed.channel = value,
                    _ => parsed.pane = value,
                }
            }
            _ => {}
        }
    }
    if elapsed != Ok(None) && parsed.long_running {
        elapsed = Err("--elapsed cannot be combined with --long-running".to_owned());
    }
    ParsedArgs {
        event: parsed,
        warnings,
        elapsed,
    }
}

#[cfg(test)]
mod tests;
