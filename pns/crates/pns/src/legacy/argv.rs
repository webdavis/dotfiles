//! pns's CLI contract is strict: bad input is refused and named.
//!
//! Four rules carry it: a value-taking flag whose next token is missing or is
//! itself a RECOGNIZED flag is refused WITHOUT consuming that token (consuming
//! it would silently drop the real flag, e.g. leak an event a caller narrowed
//! with `--pane --scope local_only`); an unrecognized next token IS taken as
//! the value, the leniency that lets a detail begin with a dash; any other
//! unknown argument is refused and named; and `--help`/`-h` sitting in FLAG
//! position wins over all three and turns the event into a usage print, while
//! the same word sitting where a flag's value belongs is still just a value,
//! under the second rule.

use pns_domain::{DeliveryScope, EventArgs};
use pns_protocol::{Remind, State};

/// Every flag that takes a value. Private: the only consumers are the
/// predicates in this module. It used to be `pub` so a test could assert the
/// hand-typed usage text mentioned every flag, a declaration-parity check
/// rather than one about this parser's behavior; that test is gone.
const VALUE_FLAGS: [&str; 12] = [
    "--producer",
    "--state",
    "--project",
    "--branch",
    "--detail",
    "--pane",
    "--route",
    "--elapsed",
    "--request-id",
    "--session",
    "--delivery-class",
    "--scope",
];

/// Every flag that takes no value. It is a LIST rather than a chain of
/// comparisons because the chain is what went stale before: a bare flag
/// handled elsewhere and never added here let a value flag in front of it eat
/// it as its value and the signal vanished without a warning.
const BARE_FLAGS: [&str; 2] = ["--remind", "--no-remind"];

/// Every flag pns used to take, paired with the one that replaced it and by
/// whether it took a value. A retired flag is REFUSED and the refusal names its
/// replacement, so a caller still typing the old spelling is told the new one
/// instead of watching its producer name vanish into the lenient skip. The flag
/// that took a value consumes it too, so `--channel priority` does not leave
/// `priority` behind as a stray word; a bare one consumes nothing, so
/// `--local-only --help` still prints the usage it asked for.
const RETIRED_FLAGS: [(&str, &str, bool); 7] = [
    ("--agent", "--producer", true),
    ("--kind", "--delivery-class", true),
    ("--channel", "--route", true),
    ("--local-only", "--scope", false),
    ("--remote-only", "--scope", false),
    ("--long-running", "--elapsed", false),
    (
        "--require-delivery",
        "the exit code, which always reports delivery now",
        false,
    ),
];

/// Whether a token is a producer flag. A retired flag counts, so a flag whose
/// value is missing (`--detail --agent x`) is refused rather than eating the
/// retired flag as its value.
fn is_producer_flag(token: &str) -> bool {
    VALUE_FLAGS.contains(&token)
        || BARE_FLAGS.contains(&token)
        || RETIRED_FLAGS.iter().any(|(retired, ..)| *retired == token)
}

/// Whether a token is a flag pns takes that this parse acts on nowhere: the
/// reminder switch, which `remind_switch` reads off the raw argv, and the
/// tool-wide colour flag, which the composition root already answered. Each
/// is recognized here so the strict unknown-argument rule does not refuse a
/// flag pns takes.
fn is_answered_elsewhere(token: &str) -> bool {
    BARE_FLAGS.contains(&token)
        || token.starts_with("--remind=")
        || token == crate::invocation::NO_COLOR_FLAG
}

/// The reminder switch a hook's own argv carried, or `None` when it named
/// neither flag.
///
/// THE VALUE IS JOINED WITH `=`, the convention of `git log --color[=<when>]`:
/// a value separated by a space is never read as the delay, so the token after
/// `--remind` is never swallowed.
///
/// THE LAST ONE WINS, so a wrapper appending its own switch overrides the one
/// it wrapped rather than being ignored by it.
pub fn remind_switch(argv: &[String]) -> Result<Option<Remind>, String> {
    let mut switch = None;
    for token in argv {
        let found = match token.as_str() {
            "--remind" => Remind::Configured,
            "--no-remind" => Remind::Off,
            other => match other.strip_prefix("--remind=") {
                Some(duration) => Remind::After(remind_after(duration)?),
                None => continue,
            },
        };
        switch = Some(found);
    }
    Ok(switch)
}

/// `--remind=<duration>`'s own value, read by the parser every other pns
/// duration goes through and held to the range `[remind] delay` is held to.
fn remind_after(duration: &str) -> Result<std::time::Duration, String> {
    pns_domain::duration::parse_duration("--remind", duration, pns_adapters::remind_delay_range())
        .map_err(|refusal| refusal.trim_start_matches("pns: ").to_owned())
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
    /// The first flag-level refusal argv earned: a retired flag, a flag
    /// given no value, or a word that is no flag of pns's at all.
    flags: Option<String>,
    /// `--state`: what happened, in one of six words. A seventh word refuses
    /// the event rather than being delivered as itself, because the state is
    /// what the lamps, the routes and the recap all read and a word none of
    /// them knows is a page nobody gets.
    state: Option<String>,
    elapsed: Result<Option<u64>, String>,
    /// `--request-id`, `--session` and `--delivery-class`: the names a JSON
    /// producer already sends, now spelled as flags. Each is held to the
    /// identifier rules the envelope holds its twin to, so one spelling
    /// cannot carry a value the other refuses.
    identifiers: Result<(), String>,
    /// `--session`: the harness session this event belongs to, which lands in
    /// the same payload field a JSON request's `session` does.
    pub session: String,
    /// `--delivery-class`: what the event IS for delivery, which decides its
    /// route when the producer named none and whether it passes a mute. The
    /// same vocabulary the JSON request's `delivery_class` takes, because one
    /// field spelled two ways is still one field.
    pub delivery_class: String,
    /// `--scope`: how wide this delivery may reach, in one of three words. One
    /// flag cannot contradict itself, which is why the pair it replaced needed
    /// a refusal for being given together and this does not.
    scope: Result<DeliveryScope, String>,
}

impl ParsedArgs {
    /// The event, or the first refusal argv earned: said on stderr, and nothing
    /// is delivered.
    pub fn into_event(self) -> Result<Option<EventArgs>, String> {
        if let Some(refusal) = self.flags {
            return Err(refusal);
        }
        if let Some(refusal) = self.state {
            return Err(refusal);
        }
        self.identifiers?;
        let elapsed = self.elapsed?;
        let event = match elapsed {
            Some(seconds) => pns_domain::elapsed_event(self.event, seconds),
            None => Some(self.event),
        };
        let Some(mut event) = event else {
            return Ok(None);
        };
        // THE DELIVERY CLASS TRAVELS, NEVER THE ROUTE IT NAMES. Which route a
        // class takes is settled once the config is read (`EventArgs::routed`),
        // because the class is the operator's (`[delivery_class.<name>]`) and
        // this parse runs before any file is opened.
        event.delivery_class = self.delivery_class;
        event.scope = self.scope?;
        Ok(Some(event))
    }
}

/// Parse argv, keeping the first refusal of each kind rather than the last.
pub(super) fn parse_args<I>(argv: I) -> ParsedArgs
where
    I: IntoIterator<Item = String>,
{
    let mut parsed = EventArgs::default();
    let mut help = false;
    let mut elapsed = Ok(None);
    let mut identifiers = Ok(());
    let mut session = String::new();
    let mut flags = None;
    let mut state = None;
    let mut delivery_class = String::new();
    let mut scope = Ok(DeliveryScope::default());
    let mut tokens = argv.into_iter().peekable();
    while let Some(token) = tokens.next() {
        match token.as_str() {
            // HELP IN FLAG POSITION WINS: this arm only ever sees a token
            // that reached the top of the loop unconsumed, so `--state
            // --help` never lands here, the value arm below already took
            // `--help` as `--state`'s value by the time this token is asked
            // about again.
            flag if is_help_flag(flag) => help = true,
            "--delivery-class" => {
                let value = tokens
                    .next_if(|next| !is_producer_flag(next))
                    .unwrap_or_default();
                match pns_protocol::Name::new(value.as_str()) {
                    Ok(_) => delivery_class = value,
                    Err(error) => {
                        identifiers = identifiers.and(Err(format!(
                            "--delivery-class is not a usable name: {error}"
                        )));
                    }
                }
            }
            "--scope" => {
                let word = tokens.next_if(|next| !is_producer_flag(next));
                if scope.is_ok() {
                    scope = word
                        .as_deref()
                        .and_then(DeliveryScope::from_word)
                        .ok_or_else(|| {
                            format!(
                                "--scope requires one of: {}",
                                DeliveryScope::WORDS.join(", ")
                            )
                        });
                }
            }
            // A DURATION, NEVER A BARE NUMBER, through the parser every other
            // pns duration goes through: `90` means seconds to one reader and
            // minutes to the next, so it is refused rather than guessed.
            "--elapsed" => {
                let value = tokens.next_if(|next| !is_producer_flag(next));
                if elapsed.is_ok() {
                    elapsed = pns_domain::duration::parse_duration(
                        "--elapsed",
                        value.as_deref().unwrap_or_default(),
                        pns_domain::elapsed::RANGE,
                    )
                    .map(|elapsed| Some(elapsed.as_secs()))
                    // The shared parser words its own refusal with the pns
                    // prefix; this path prints one of its own.
                    .map_err(|refusal| refusal.trim_start_matches("pns: ").to_owned());
                }
            }
            "--request-id" => {
                let value = tokens
                    .next_if(|next| !is_producer_flag(next))
                    .unwrap_or_default();
                match pns_protocol::RequestId::new(value.clone()) {
                    Ok(_) => parsed.request_id = value,
                    Err(error) => {
                        identifiers = identifiers
                            .and(Err(format!("--request-id is not a usable id: {error}")));
                    }
                }
            }
            "--session" => {
                let value = tokens
                    .next_if(|next| !is_producer_flag(next))
                    .unwrap_or_default();
                match pns_protocol::Name::new(value.clone()) {
                    Ok(_) => session = value,
                    Err(error) => {
                        identifiers = identifiers
                            .and(Err(format!("--session is not a usable name: {error}")));
                    }
                }
            }
            // ITS OWN ARM, like `--delivery-class` and `--elapsed` above,
            // rather than the generic value flag below: a missing value refuses
            // the same way an out-of-set word does, instead of warning and
            // delivering an event with no state at all.
            "--state" => {
                let value = tokens.next_if(|next| !is_producer_flag(next));
                if State::from_word(value.as_deref().unwrap_or_default()).is_none() {
                    state.get_or_insert(format!(
                        "--state requires one of: {}",
                        State::WORDS.join(", ")
                    ));
                }
                parsed.state = value.unwrap_or_default();
            }
            flag if VALUE_FLAGS.contains(&flag) => {
                // Missing, or a recognized flag standing where the value
                // should be: refuse and leave the token for its own arm.
                if tokens.peek().is_none_or(|next| is_producer_flag(next)) {
                    flags.get_or_insert_with(|| format!("{flag} requires a value"));
                    continue;
                }
                let Some(value) = tokens.next() else { continue };
                match flag {
                    "--producer" => parsed.agent = value,
                    "--project" => parsed.project = value,
                    "--branch" => parsed.branch = value,
                    "--detail" => parsed.detail = value,
                    "--route" => parsed.channel = value,
                    _ => parsed.pane = value,
                }
            }
            token if is_answered_elsewhere(token) => {}
            _ => {
                match RETIRED_FLAGS.iter().find(|(flag, ..)| *flag == token) {
                    Some((flag, replacement, takes_value)) => {
                        // ITS VALUE GOES WITH IT: leaving `codex` behind would
                        // make the value a second refusal of its own.
                        if *takes_value {
                            tokens.next_if(|next| !is_producer_flag(next));
                        }
                        flags
                            .get_or_insert_with(|| format!("{flag} was replaced by {replacement}"));
                    }
                    // STRICT, AND THE SAME SHAPE THE JSON PATH REFUSES AN
                    // UNKNOWN FIELD WITH: a word pns skipped in silence was a
                    // caller whose narrowing, detail or route went nowhere.
                    None => {
                        flags.get_or_insert_with(|| format!("{token} is not a flag pns takes"));
                    }
                }
            }
        }
    }
    ParsedArgs {
        help,
        event: parsed,
        flags,
        state,
        elapsed,
        identifiers,
        session,
        delivery_class,
        scope,
    }
}

#[cfg(test)]
mod tests;
