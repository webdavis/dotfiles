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

use pns_domain::{DeliveryScope, EventArgs, routes::Kind};

/// Every flag that takes a value. Private: the only consumers are the
/// predicates in this module. It used to be `pub` so a test could assert the
/// hand-typed usage text mentioned every flag, a declaration-parity check
/// rather than one about this parser's behavior; that test is gone.
const VALUE_FLAGS: [&str; 9] = [
    "--producer",
    "--state",
    "--project",
    "--branch",
    "--detail",
    "--pane",
    "--channel",
    "--elapsed",
    "--kind",
];

/// Every flag that takes no value. It is a LIST rather than a chain of
/// comparisons because the chain is what went stale: `--long-running` was
/// handled below and never added here, so a value flag in front of it ate it as
/// its value and the tier vanished without a warning.
const BARE_FLAGS: [&str; 4] = [
    "--long-running",
    "--local-only",
    "--remote-only",
    "--require-delivery",
];

/// Every flag pns used to take, paired with the one that replaced it. A
/// retired flag is REFUSED and the refusal names its replacement, so a caller
/// still typing the old spelling is told the new one instead of watching its
/// producer name vanish into the lenient skip.
const RETIRED_FLAGS: [(&str, &str); 1] = [("--agent", "--producer")];

/// Whether a token is a producer flag. A retired flag counts, so a flag whose
/// value is missing (`--detail --agent x`) is warned about rather than eating
/// the retired flag as its value.
fn is_producer_flag(token: &str) -> bool {
    VALUE_FLAGS.contains(&token)
        || BARE_FLAGS.contains(&token)
        || RETIRED_FLAGS.iter().any(|(retired, _)| *retired == token)
}

/// Whether a token is `--help`/`-h`.
///
/// PUBLIC BECAUSE THE COMPOSITION ROOT ASKS TOO: `pns --help` with no
/// subcommand behind it prints instead of refusing, and a second copy of the
/// two spellings in `main` is exactly the drift the `--long-running` bug above
/// came from.
pub fn is_help_flag(token: &str) -> bool {
    token == "--help" || token == "-h"
}

pub(super) struct ParsedArgs {
    pub help: bool,
    pub event: EventArgs,
    pub warnings: Vec<String>,
    /// `--require-delivery`: whether this caller wants the exit code to say
    /// that its page did not reach the durable log.
    ///
    /// OPT-IN, AND IT HAS TO BE. Decision 0010 says a notification never fails
    /// the work it reports on, and every harness hook, the shell notifier and
    /// the daemon call this while real work is in flight. A caller that asked
    /// for the answer is a caller that can take it; every other one keeps the
    /// exit-0 contract untouched.
    pub require_delivery: bool,
    /// The first retired flag argv carried, already worded as its refusal.
    retired: Option<String>,
    elapsed: Result<Option<u64>, String>,
    /// `--kind`: what the event IS, which decides its route when the producer
    /// named none. A word that is neither kind refuses the event rather than
    /// falling back to the default, the same way a bad `--elapsed` does.
    kind: Result<Kind, String>,
    scope: Option<DeliveryScope>,
}

pub(super) enum Refusal {
    Scope,
    /// A flag value pns refuses: said on stderr, and nothing is delivered.
    Value(String),
}

impl ParsedArgs {
    pub fn into_event(self) -> Result<Option<EventArgs>, Refusal> {
        if let Some(retired) = self.retired {
            return Err(Refusal::Value(retired));
        }
        let elapsed = self.elapsed.map_err(Refusal::Value)?;
        let kind = self.kind.map_err(Refusal::Value)?;
        let event = match elapsed {
            Some(seconds) => pns_domain::elapsed_event(self.event, seconds),
            None => Some(self.event),
        };
        let Some(mut event) = event else {
            return Ok(None);
        };
        // THE KIND TRAVELS, NEVER THE ROUTE IT NAMES. Which route a kind takes
        // is settled once the config is read (`EventArgs::routed`), because the
        // route's NAME is the operator's (`[routes] urgent`) and this parse
        // runs before any file is opened.
        event.kind = kind;
        event.scope = self.scope.ok_or(Refusal::Scope)?;
        Ok(Some(event))
    }
}

/// Parse argv, retaining legacy warnings and a separate elapsed refusal.
pub(super) fn parse_args<I>(argv: I) -> ParsedArgs
where
    I: IntoIterator<Item = String>,
{
    let mut parsed = EventArgs::default();
    let mut help = false;
    let mut local_only = false;
    let mut remote_only = false;
    let mut require_delivery = false;
    let mut warnings = Vec::new();
    let mut elapsed = Ok(None);
    let mut retired = None;
    let mut kind = Ok(Kind::default());
    let mut tokens = argv.into_iter().peekable();
    while let Some(token) = tokens.next() {
        match token.as_str() {
            "--long-running" => parsed.long_running = true,
            "--local-only" => local_only = true,
            "--remote-only" => remote_only = true,
            "--require-delivery" => require_delivery = true,
            // HELP IN FLAG POSITION WINS: this arm only ever sees a token
            // that reached the top of the loop unconsumed, so `--state
            // --help` never lands here, the value arm below already took
            // `--help` as `--state`'s value by the time this token is asked
            // about again.
            flag if is_help_flag(flag) => help = true,
            "--kind" => {
                let word = tokens.next_if(|next| !is_producer_flag(next));
                if kind.is_ok() {
                    kind = word.as_deref().and_then(Kind::from_word).ok_or_else(|| {
                        format!("--kind requires one of: {}", Kind::WORDS.join(", "))
                    });
                }
            }
            "--elapsed" => {
                let value = tokens.next_if(|next| !is_producer_flag(next));
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
                    "--producer" => parsed.agent = value,
                    "--state" => parsed.state = value,
                    "--project" => parsed.project = value,
                    "--branch" => parsed.branch = value,
                    "--detail" => parsed.detail = value,
                    "--channel" => parsed.channel = value,
                    _ => parsed.pane = value,
                }
            }
            _ => {
                if let Some((flag, replacement)) =
                    RETIRED_FLAGS.iter().find(|(flag, _)| *flag == token)
                {
                    // ITS VALUE GOES WITH IT: leaving `codex` behind would
                    // make the next unknown-token rule read it as a stray word.
                    tokens.next_if(|next| !is_producer_flag(next));
                    retired.get_or_insert_with(|| format!("{flag} was replaced by {replacement}"));
                }
            }
        }
    }
    if elapsed != Ok(None) && parsed.long_running {
        elapsed = Err("--elapsed cannot be combined with --long-running".to_owned());
    }
    ParsedArgs {
        help,
        event: parsed,
        warnings,
        require_delivery,
        retired,
        elapsed,
        kind,
        scope: match (local_only, remote_only) {
            (false, false) => Some(DeliveryScope::Automatic),
            (true, false) => Some(DeliveryScope::LocalOnly),
            (false, true) => Some(DeliveryScope::RemoteOnly),
            (true, true) => None,
        },
    }
}

#[cfg(test)]
mod tests;
