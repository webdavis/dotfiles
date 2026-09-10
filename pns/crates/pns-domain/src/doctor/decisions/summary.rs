//! One recorded decision, said in a sentence.
//!
//! THE RING'S OWN FORMAT IS `<epoch> <agent>/<state> <key=value ...>`, written
//! by the adapter that keeps the log. This reads it back. The two used to be
//! decoupled on purpose, the reader printing the body verbatim so a writer
//! change needed no matching change here, and the price was the report: every
//! entry arrived as twenty-five `key=value` pairs on one line, which nobody
//! reads. Coupling to the format is what buys a sentence.
//!
//! IT NEVER GUESSES. A field this does not find is a fact it does not state,
//! so an entry written by a future version says less rather than something
//! untrue, and the raw line is one flag away for anyone who needs all of it.

/// What a decision did, and the one reason that explains it.
pub struct Summary {
    /// `agent/state`, as the ring recorded it.
    pub event: String,
    /// What was sent, in the reader's words.
    pub outcome: String,
    /// Why it came out that way, when a field explains it.
    pub because: Option<String>,
}

/// Read one entry's body: everything after the epoch.
pub fn summarize(body: &str) -> Summary {
    let (event, rest) = body.split_once(' ').unwrap_or((body, ""));
    let fields = Fields(rest);
    Summary {
        event: event.to_string(),
        outcome: outcome(&fields),
        because: because(&fields),
    }
}

/// A borrowed view over the `key=value` run, read by scanning rather than
/// collected into a map: there are two dozen fields and at most a handful are
/// ever asked for, so a map would allocate for every entry to answer three
/// lookups.
struct Fields<'a>(&'a str);

impl<'a> Fields<'a> {
    fn get(&self, key: &str) -> Option<&'a str> {
        self.0.split(' ').find_map(|pair| {
            let (name, value) = pair.split_once('=')?;
            (name == key).then_some(value)
        })
    }

    /// A `yes`/`no` field, with anything else read as absent rather than as
    /// false: an unrecognized value is not evidence for either answer.
    fn yes(&self, key: &str) -> bool {
        self.get(key) == Some("yes")
    }
}

/// What actually went out.
///
/// THE PLAN IS WHAT WAS INTENDED and `legs` is what happened, so the legs win
/// where they exist. A plan of three with one delivered leg would otherwise
/// report three sends that never landed.
fn outcome(fields: &Fields<'_>) -> String {
    let delivered = fields
        .get("legs")
        .filter(|legs| !legs.is_empty() && *legs != crate::ABSENT)
        .map(|legs| {
            legs.split(',')
                .filter_map(|leg| leg.split_once(':').map(|(channel, _)| channel))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !delivered.is_empty() {
        return format!("reached {}", spoken_list(&delivered));
    }

    let planned = fields.get("plan").map(|plan| {
        plan.split(',')
            .filter_map(|part| part.split_once(':'))
            .filter(|(_, fired)| *fired == "yes")
            .map(|(what, _)| match what {
                "banner" => "a banner",
                "card" => "a phone card",
                "pulse" => "the lamps",
                other => other,
            })
            .collect::<Vec<_>>()
    });
    match planned {
        Some(planned) if !planned.is_empty() => format!("planned {}", spoken_list(&planned)),
        // NOTHING SENT IS A RESULT, not an absence. It is the answer the
        // operator most often opens this log to explain.
        _ => "nothing sent".to_string(),
    }
}

/// The one field that explains the outcome, in the order that decides it.
///
/// FIRST MATCH WINS AND THE ORDER IS THE POLICY'S. An operator mute outranks a
/// Focus mode, which outranks where they were, because that is the order the
/// decision itself applies them; naming a later reason while an earlier one was
/// in force would send them to the wrong switch.
fn because(fields: &Fields<'_>) -> Option<String> {
    if fields.yes("muted") {
        return Some("`pns quiet` was running".into());
    }
    if fields.yes("focus") {
        return Some("a Focus mode pns respects was on".into());
    }
    if fields.yes("local_only") {
        return Some("the event asked for this Mac only".into());
    }
    if fields.yes("remote_only") {
        return Some("the event asked for the phone only".into());
    }
    if fields.yes("nag") {
        return Some("it was a repeat of an approval nobody answered".into());
    }
    where_you_were(fields)
}

/// Where the decision believed the operator was, which is what settles most
/// entries.
fn where_you_were(fields: &Fields<'_>) -> Option<String> {
    match fields.get("surface")? {
        // THE PANE CLAUSE NEEDS THE FIELD THAT SUPPORTS IT. Reading an absent
        // `visibility` as "not Visible" would print "but the pane was hidden"
        // over an entry that never said so, which is the report inventing the
        // very thing the reader came to check.
        "Desk" => Some(match fields.get("visibility") {
            Some("Visible") => "you were at the desk with the pane in view".into(),
            Some("Hidden") => "you were at the desk, but the pane was hidden".into(),
            _ => "you were at the desk".to_string(),
        }),
        "Away" => Some("you were away from the Mac".into()),
        "Phone" => Some("you were on the phone rather than the Mac".into()),
        _ => None,
    }
}

/// `a`, `a and b`, `a, b and c`.
fn spoken_list(items: &[&str]) -> String {
    match items {
        [] => String::new(),
        [only] => (*only).to_string(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

#[cfg(test)]
#[path = "summary/tests.rs"]
mod tests;
