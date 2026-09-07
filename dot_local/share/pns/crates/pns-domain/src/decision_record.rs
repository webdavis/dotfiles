//! What one decision line SAYS, as values: the verdicts each leg answered
//! with, the readings that may be absent, and the only text a line carries.
//!
//! THE LINE'S OWN SHAPE IS NOT HERE. How those values are laid out as
//! `<epoch> <key=value ...>`, and how the ring is read back, is the state
//! file's business and stays with it. This module is what each field is
//! allowed to hold.

use crate::routing::{Delivery, Leg};

/// How many decisions the ring keeps, which is also how many the report
/// prints. ONE CONSTANT for both, so the file holds exactly what is read.
///
/// FIVE RATHER THAN ONE, because a single slot does not survive being looked
/// at: between the card the operator wondered about and them typing
/// `pns doctor`, the Stop hook of the session they are typing in fires its own
/// event and overwrites it. Five covers that card through a couple of
/// intervening turns. Raising it is this one number.
pub const KEPT: usize = 5;

/// One `plugin:verdict` per dispatched leg, in delivery order.
///
/// THE VARIANT NAME AND NEVER THE SENTENCE. A channel's own words can carry a
/// status code or a URL, and this file is printed by `pns doctor`; the variant
/// is the verdict anyway, which is why `Delivery` keeps the two apart. The
/// plugin name comes out of the compiled roster, so nothing here can carry a
/// newline.
pub fn verdicts(legs: &[(Leg, Delivery)]) -> String {
    if legs.is_empty() {
        return ABSENT.to_string();
    }
    legs.iter()
        .map(|(leg, delivery)| {
            let verdict = match delivery {
                Delivery::Delivered(_) => "delivered",
                Delivery::Failed(_) => "failed",
                Delivery::Unlaunched(_) => "unlaunched",
                Delivery::Silent => "silent",
            };
            format!("{}:{verdict}", leg.name)
        })
        .collect::<Vec<String>>()
        .join(",")
}

/// The only text a line carries, filtered to what may be PRINTED.
///
/// `agent` and `state` come from argv and are what identify which card the
/// operator is asking about, so they cannot be reduced to a boolean the way
/// the rest of the event is. Everything else on a line is a number, a boolean,
/// an enum name or a plugin name out of the compiled roster.
///
/// A NEWLINE IS THE ONE THAT MATTERS: this file is one record per line, so a
/// value carrying one FORGES a second entry that the reader cannot tell from a
/// real decision. An escape sequence is the other, because `pns doctor` prints
/// these straight to a terminal.
///
/// DELIBERATELY NOT `safety::route_name_is_usable`. That predicate's doc
/// comment says it exists so ONE rule judges route names, and borrowing it for
/// "what may be printed into a report" would make it two rules wearing one
/// spelling: they would then be changed for one caller and silently applied to
/// the other. This is the new rule, and printing is what it is for.
///
/// THE WHOLE VALUE IS JUDGED BEFORE ANYTHING IS TRUNCATED, which is also what
/// makes the truncation safe: every accepted byte is ASCII, so a cut at
/// `IDENTITY_MAX` can never land inside a multi-byte character.
pub fn printable(text: &str) -> String {
    if text.is_empty() {
        return ABSENT.to_string();
    }
    if !text
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_'))
    {
        return UNPRINTABLE.to_string();
    }
    text.chars().take(IDENTITY_MAX).collect()
}

/// What a value outside the allowlist is recorded as. The line still names
/// the decision it belonged to, which is more than dropping the entry would.
const UNPRINTABLE: &str = "unprintable";

/// The longest agent or state a line carries. Both are short names in every
/// producer this repo owns; the cap is what stops an argv nobody validated
/// from filling the ring with one entry.
const IDENTITY_MAX: usize = 32;

/// A reading nobody could take, spelled the one way everywhere in a line. It
/// is never a zero: `0` reads as "touched this instant", which is a claim
/// about a measurement that never happened.
pub const ABSENT: &str = "none";

pub fn count(reading: Option<u64>) -> String {
    reading.map_or_else(|| ABSENT.to_string(), |value| value.to_string())
}

/// A boolean reading that may also be absent, which is three states and never
/// two: an unread lock is not an unlocked one.
pub fn tri(reading: Option<bool>) -> &'static str {
    match reading {
        Some(locked) => yes_no(locked),
        None => ABSENT,
    }
}

pub fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}
