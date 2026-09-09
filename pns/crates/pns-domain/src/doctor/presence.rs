use super::pairing::printable;

/// The room sensor's line, in every state the reading can be in.
///
/// UNKNOWN NAMES WHICH KIND OF UNKNOWN, because the five are five different
/// things to go and fix: nothing published yet, a daemon or a bridge that
/// stopped, a clock, a wrong epoch, and a room the config does not watch.
pub(super) fn presence_said(
    plugin: &str,
    status: &crate::presence::PresenceStatus,
    last_narrowing: Option<&crate::PresenceDecision>,
) -> String {
    use crate::presence::PresenceStatus;
    let reading = match status {
        PresenceStatus::Room { room, age_secs } => {
            format!("{} ({age_secs}s ago)", shown_room(room))
        }
        // THE BRIDGE ANSWERED AND ANSWERED "NOT THERE", which is a different
        // fact from not knowing and is worth its own word.
        PresenceStatus::Nowhere { poll_age_secs } => {
            format!("nowhere (poll {poll_age_secs}s ago)")
        }
        PresenceStatus::Unknown(reason) => {
            format!("unknown ({})", crate::presence::unreadable_said(reason))
        }
    };
    // WHAT THE LAMPS DID WITH IT, which the reading alone does not say: the
    // desk overrules a room, a room holding no lamp falls back, and an
    // operator staring at a lamp in the wrong room needs to see which. It is
    // read back out of a state file, so it crosses the same filter the room
    // name above does.
    match last_narrowing {
        Some(narrowing) => format!(
            "{plugin}: {reading}; last narrowed {}",
            match &narrowing.room {
                Some(room) => format!("to {}", shown_room(room)),
                None => format!("nothing ({})", printable(&narrowing.reason)),
            }
        ),
        None => format!("{plugin}: {reading}"),
    }
}

/// The room name, made safe to put on a terminal.
///
/// THE BRIDGE CHOSE THIS TEXT, exactly as moshi chose the sentence beside it,
/// so it crosses the same filter: an unfiltered newline in a room name forges
/// a second `pns doctor:` line the operator would read as pns's own verdict.
/// A name that filters away to nothing is NAMED AS SUCH rather than printed
/// as a blank, which would read as a room whose name is empty.
fn shown_room(room: &str) -> String {
    let shown = printable(room);
    if shown.trim().is_empty() {
        return "a room whose name will not print".to_string();
    }
    shown
}
