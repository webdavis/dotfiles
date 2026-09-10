/// The decision ring, read back and rendered.
///
/// READ AND NEVER APPENDED. A doctor that recorded would push the decision the
/// operator came to read out of the ring by the act of going to look at it.
pub(super) fn decision_section(
    records: &impl crate::DecisionRing,
    now: Option<u64>,
    detail: pns_domain::doctor::Detail,
) -> Vec<(pns_domain::doctor::Mark, String)> {
    match records.read() {
        Ok(contents) => pns_domain::doctor::decision_section(contents.as_deref(), now, detail),
        Err(kind) => vec![(
            pns_domain::doctor::Mark::Note,
            format!("{DECISIONS_UNREADABLE} ({kind})."),
        )],
    }
}

/// A ring that is there and cannot be read. Said HERE rather than in the log
/// module, for the reason `NO_HUE_BRIDGE_LINE` is: the sentence needs
/// something only the reader of the file knows.
const DECISIONS_UNREADABLE: &str = "pns doctor: the decision log could not be read";

/// The missed-notification journal, COUNTED and never rendered.
///
/// READ AND NEVER APPENDED, for the reason the decision section is: a doctor
/// that journaled would file a miss for the act of going to look for one, and
/// its own test send is the last event anything should ever replay.
///
/// NOTHING HERE PARSES AN ENTRY. The contents go straight to `waiting_line`,
/// which counts lines and has no parse at all, so the operator's own text has
/// no path from this file to a terminal.
///
/// `replay_card` REACHES THE SENTENCE because the sentence makes a promise.
/// With the card switched off nothing will ever deliver what is counted here,
/// and a doctor that still named "the next event" would be telling the
/// operator a lie their own setting makes permanent.
pub(super) fn missed_line(records: &impl crate::Journal, replay_card: bool) -> String {
    match records.read() {
        Ok(contents) => pns_domain::missed::waiting_line(contents.as_deref(), replay_card),
        Err(kind) => format!("{MISSED_UNREADABLE} ({kind})."),
    }
}

/// A journal that is there and cannot be read. Said HERE rather than in the
/// module, for the reason `DECISIONS_UNREADABLE` is.
const MISSED_UNREADABLE: &str = "pns doctor: the missed-notification journal could not be read";
