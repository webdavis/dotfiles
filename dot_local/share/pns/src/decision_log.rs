//! The decision log: why a card did or did not fire, in one line per event.
//!
//! POLICY ONLY, in `doctor.rs`'s style: every function here is a total
//! function of its arguments, with no config, no clock, no environment, no
//! file and no printing. The composition root reads the world, assembles a
//! `Record` out of values the decision ALREADY HAS, appends what comes back
//! and prints what the doctor asks for. This module never learns where the
//! file is.
//!
//! THE RECORD IS THE DECISION'S OWN READINGS, never a second reading taken
//! afterwards. That is why a `Record` is built from a `&Decision` rather than
//! from loose values: two readings of where the operator is can disagree, and
//! an explanation assembled from the later one describes a moment the decision
//! never saw. `engine` owns `GateInputs` for the same reason, and this module
//! depends on it rather than the other way round. The other three types it
//! names (`EventArgs`, `Leg`, `Delivery`) are the crate's own value types,
//! taken exactly as the composition root already holds them so that nothing is
//! transformed on the way in.

/// The record's VALUES moved to `pns-domain`: how many entries the section
/// keeps, the verdict per leg, the readings that may be absent, and the only
/// text a line carries. What stays here is the line's own shape and the
/// section that reads it back.
pub use pns_domain::decision_record::{ABSENT, KEPT, count, printable, tri, verdicts, yes_no};

/// One decision, as everything needed to write its line. THE STRUCT IS THE
/// SCHEMA, and it moved to `pns-domain` once every field it borrows did: the
/// use case that orders the write holds one, and `line` below turns it into
/// the ring's own text.
pub use pns_domain::decision_record::Record;

/// One decision as one line: `<epoch> <key=value ...>`.
///
/// NO FREE TEXT REACHES IT. The detail, the branch, the project and the pane
/// id are the operator's own content, and this file is printed to a terminal
/// by `pns doctor`, so recording them would put that content into a state file
/// and then onto a screen. The pane appears as the two booleans the decision
/// actually used it for. Every other value here is a number, a boolean, an
/// enum name or a plugin name out of the compiled roster.
///
/// NOT JSON, though the crate already carries a JSON writer. The only reader
/// is the section below, whose whole parse is one `split_once(' ')` over the
/// epoch; a JSON round trip would add a schema and an error taxonomy to render
/// a sentence that reads as it stands.
///
/// NO actionId IS RECORDED, because pns never has one: the notification seam
/// answers a bool and drops the body, and on the approval path moshi mints the
/// id inside itself and answers with an exit code.
pub fn line(record: &Record) -> String {
    let inputs = record.decision.inputs;
    let overrides = record.overrides;
    format!(
        "{epoch} {agent}/{state} \
         mode={mode} agent={payload_agent} tool={tool} \
         surface={surface:?} visibility={visibility:?} \
         session_visibility={session_visibility:?} \
         desk_age={desk_age} phone_age={phone_age} tap_age={tap_age} \
         locked={locked} fresh_window={fresh_window} long_running={long_running} \
         nag={nag} \
         local_only={local_only} remote_only={remote_only} \
         pane={pane} pane_dropped={pane_dropped} watch_card={watch_card} \
         muted={muted} focus={focus} skip_phone={skip_phone} force_phone={force_phone} \
         idle_invalid={idle_invalid} desk_invalid={desk_invalid} \
         phone_invalid={phone_invalid} \
         plan=banner:{banner},card:{card},pulse:{pulse} legs={legs}",
        epoch = inputs
            .now_secs
            .map_or_else(|| NO_CLOCK.to_string(), |now| now.to_string()),
        agent = printable(&record.event.agent),
        state = printable(&record.event.state),
        // THE PAYLOAD'S OWN IDENTITY, PRINTABLE-FILTERED LIKE `agent`/`state`
        // ABOVE: `tool_name` is remote text a connected MCP server named, so
        // it gets the same allowlist rather than a free pass into a file
        // `pns doctor` prints straight to a terminal.
        mode = printable(record.permission_mode),
        payload_agent = printable(record.agent_id),
        tool = printable(record.tool_name),
        // A FIELDLESS DERIVED `Debug` IS THE VARIANT NAME, which is exactly
        // what an enum reads as here.
        surface = inputs.surface,
        visibility = inputs.visibility,
        session_visibility = inputs.session_visibility,
        desk_age = count(inputs.desk_input_age),
        phone_age = count(inputs.phone_input_age),
        tap_age = count(inputs.marker_age),
        locked = tri(inputs.screen_locked),
        fresh_window = count(inputs.desk_fresh_secs),
        long_running = yes_no(inputs.long_running),
        nag = yes_no(record.nag),
        local_only = yes_no(inputs.local_only),
        remote_only = yes_no(inputs.remote_only),
        // THE PANE AS THE DECISION USED IT and no further: its value is a
        // multiplexer id this crate does not own, and these two booleans are
        // everything the decision read out of it.
        pane = if inputs.pane_present {
            "present"
        } else {
            ABSENT
        },
        pane_dropped = yes_no(record.decision.pane_dropped),
        watch_card = yes_no(inputs.mobile_watch_card),
        muted = yes_no(overrides.muted),
        // TWO FIELDS RATHER THAN ONE. The log exists to answer "why did no
        // card fire", and "you have a `pns quiet` running" sends the operator
        // somewhere completely different from "your Mac is in a Focus you told
        // pns to respect".
        focus = yes_no(overrides.focus_active),
        skip_phone = yes_no(overrides.skip_phone),
        force_phone = yes_no(overrides.force_phone),
        idle_invalid = yes_no(overrides.idle_invalid),
        desk_invalid = yes_no(overrides.desk_invalid),
        phone_invalid = yes_no(overrides.phone_invalid),
        banner = yes_no(record.decision.plan.banner),
        card = yes_no(record.decision.plan.phone_card),
        pulse = yes_no(record.decision.plan.pulse),
        legs = verdicts(record.legs),
    )
}

/// A clock nobody could read, in the field an epoch second would hold. It is
/// a RECOGNIZED value rather than epoch zero, which would parse cleanly and
/// render as 56 years ago, and rather than an empty field, which the reader
/// could not tell from a line it failed to parse.
const NO_CLOCK: &str = "-";

/// The decision log as the doctor's own section: a heading and one rendered
/// entry per line, newest first, capped at `KEPT`.
///
/// `contents` is the file, `None` when there is none. `now` is the reader's
/// own clock, which ages the entries and nothing else.
///
/// IT REPORTS HISTORY, NEVER HEALTH. Nothing here reaches an exit code: an
/// empty log on a fresh machine is not a failure, and neither is one this
/// could not parse.
pub fn section(contents: Option<&str>, now: Option<u64>) -> Vec<String> {
    let entries: Vec<&str> = contents
        .unwrap_or_default()
        .lines()
        .filter(|entry| !entry.trim().is_empty())
        .collect();
    if entries.is_empty() {
        return vec![NOTHING_RECORDED.to_string()];
    }
    // NEWEST FIRST, because the ring is written by APPEND and the operator
    // came to look at the card that just did or did not arrive.
    let shown: Vec<&&str> = entries.iter().rev().take(KEPT).collect();
    let mut rendered = vec![heading(shown.len())];
    rendered.extend(shown.into_iter().map(|entry| render(entry, now)));
    rendered
}

/// The section's own first line, counting what it is ABOUT TO SHOW rather than
/// the cap: a heading claiming five over one entry invents four decisions.
fn heading(shown: usize) -> String {
    let counted = if shown == 1 {
        "the last decision,".to_string()
    } else {
        format!("the last {shown} decisions,")
    };
    format!("pns doctor: {counted}{HEADING_TAIL}")
}

/// The rest of every heading. THE actionId IS TOLD HONESTLY rather than
/// printed as an empty field: pns never has one, because moshi mints it inside
/// the approval round trip and answers with an exit code.
const HEADING_TAIL: &str = " newest first (why a card did or did not fire). No actionId \
     is recorded: moshi mints it inside the approval round trip and never hands it back.";

/// One entry, as its age and the rest of the line it was written as.
///
/// THE BODY IS NEVER PARSED, only ESCAPED and printed. The whole reader is
/// the one split below, which is what keeps a format change in `line` from
/// needing a matching change here.
fn render(entry: &str, now: Option<u64>) -> String {
    let Some((stamp, rest)) = entry.split_once(' ') else {
        return complaint(entry);
    };
    let recorded = if stamp == NO_CLOCK {
        None
    } else {
        match crate::parse_count(stamp) {
            Some(recorded) => Some(recorded),
            // NOT DROPPED SILENTLY: a log that hides the one entry that
            // mattered is worse than one that says it cannot read it.
            None => return complaint(entry),
        }
    };
    format!("  {}: {}", age(recorded, now), escaped(rest))
}

/// THE ONE ESCAPE RULE for text out of the ring, and the reason it is a
/// function rather than two spellings: a parsed entry and an unparsable one
/// go to the SAME terminal, so a rule applied to only one of them is a rule
/// the other arm quietly does not have. Measured before this existed: an
/// entry whose epoch parsed printed its ESC and BEL bytes to the terminal
/// raw, while its unparsable neighbour on the line above was escaped.
///
/// Rust's own debug escaping is the rule, without the quotes `complaint`
/// wraps its half in: nothing here writes a format, it makes a control byte
/// visible as the characters that spell it.
fn escaped(text: &str) -> String {
    text.escape_debug().to_string()
}

/// How long ago, in the largest unit that still reads as a count. Absent at
/// either end means NO AGE IS INVENTED: an entry written with no clock, and a
/// reader with no clock of its own, are both unknowable rather than zero.
fn age(recorded: Option<u64>, now: Option<u64>) -> String {
    let (Some(recorded), Some(now)) = (recorded, now) else {
        return UNKNOWN_AGE.to_string();
    };
    let seconds = now.saturating_sub(recorded);
    match seconds {
        ..60 => format!("{seconds}s ago"),
        60..3_600 => format!("{}m ago", seconds / 60),
        _ => format!("{}h ago", seconds / 3_600),
    }
}

/// An entry this cannot read, QUOTED AND TRUNCATED. The quotes are this arm's
/// own, marking off a fragment of a file from the sentence around it; the
/// escaping inside them is `escaped`, the same rule the readable arm runs.
fn complaint(entry: &str) -> String {
    let held: String = entry.chars().take(QUOTED_MAX).collect();
    format!("  unreadable entry: \"{}\"", escaped(&held))
}

/// How much of an unreadable entry is quoted back. Enough to recognize, short
/// enough that a file of garbage cannot fill the report.
const QUOTED_MAX: usize = 60;

/// A reader with no clock of its own, and an entry written without one.
const UNKNOWN_AGE: &str = "age unknown";

/// What an absent log says. THE PARENTHESIS IS THE HONEST HALF: the write is
/// fail-quiet, so nothing here can tell an unused log from one that could not
/// be written, and the line must not claim the first.
const NOTHING_RECORDED: &str = "pns doctor: no decision has been recorded yet \
     (no event has run since this was installed, or none could be written).";

#[cfg(test)]
mod fixtures;
#[cfg(test)]
mod line_tests;
#[cfg(test)]
mod section_tests;
