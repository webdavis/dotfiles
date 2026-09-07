use super::super::instant::instant_from_utc;
use super::super::state_file::Edge;

/// What one `grouped_motion` entry says about the watched rooms.
///
/// THREE ANSWERS RATHER THAN AN `Option`, because the third one changes what
/// gets published. An entry dropped for being unreadable used to leave the
/// poll-only line, and `classify` reads that as a FRESH "nowhere": a bridge
/// serving the same garbage every few seconds kept a false absence fresh for
/// as long as it kept serving it. Refusing the poll instead lets the last good
/// reading age out into `Stale`, which is the direction every other unknown in
/// this feature takes.
pub(super) enum EntryEdge {
    /// A watched room's edge, with the full-precision instant it happened at
    /// beside the whole-second one the state file carries.
    Found { at: (u64, u32), edge: Edge },
    /// A watched room carrying a report this cannot read. The whole poll goes.
    Malformed,
    /// Everything that is neither: the house roll-up, a room nobody watches, a
    /// room the listing does not name, and a watched room whose sensors report
    /// nothing at all.
    Irrelevant,
}

/// The `.data[]` array of a CLIP response, or `None` for a body that has none.
///
/// ITS OWN COPY of `channels::hue`'s private helper, and deliberately not the
/// same function: that one answers an empty list for a body it could not read,
/// because a pulse with nothing to write is a no-op either way. Here the two
/// are different answers, and the difference is what a poll publishes.
pub(super) fn data(clip_json: &str) -> Option<Vec<serde_json::Value>> {
    let body: serde_json::Value = serde_json::from_str(clip_json).ok()?;
    Some(body.get("data")?.as_array()?.clone())
}

/// One `grouped_motion` entry, as whichever of the three answers it is.
pub(super) fn edge_of(
    entry: &serde_json::Value,
    rooms: &[serde_json::Value],
    watched: &[String],
) -> EntryEdge {
    let Some(room) = watched_room(entry, rooms, watched) else {
        return EntryEdge::Irrelevant;
    };
    // THE BRIDGE DISOWNING ITS OWN REPORT, which is read BEFORE the report is
    // parsed because it says the report is not evidence whatever shape it is
    // in. `motion_valid: false` beside a complete report used to be a Found
    // edge, and the newest edge wins, so a room the bridge vouched for nothing
    // in could name where the operator is; beside a partial one it was
    // Malformed, which refuses the whole poll and throws away every sibling
    // room that DID report.
    //
    // IT IS THE SWITCHED-OFF ANSWER, not a refusal: this room said nothing and
    // the others still count. EXPLICITLY FALSE and nothing else, so a
    // `motion_valid` that is missing, or is not a boolean, leaves the report to
    // be read exactly as before.
    if entry
        .pointer("/motion/motion_valid")
        .and_then(serde_json::Value::as_bool)
        == Some(false)
    {
        return EntryEdge::Irrelevant;
    }
    // ABSENT FOR A ROOM WHOSE SENSORS ARE OFF, which serves `motion: {}`: no
    // report is no edge, never an edge at epoch zero, and never a refusal
    // either. It is the documented shape of a switched-off sensor, so the poll
    // it belongs to is a real answer with one room quiet in it.
    let Some(report) = entry.pointer("/motion/motion_report") else {
        return EntryEdge::Irrelevant;
    };
    // PAST THIS POINT A WATCHED ROOM SAID SOMETHING, so anything unreadable in
    // it is `Malformed` rather than silence.
    match report_edge(report, room) {
        Some(found) => found,
        None => EntryEdge::Malformed,
    }
}

/// The name of the watched room this entry belongs to, or `None` when it
/// belongs to none.
///
/// EVERY REFUSAL HERE IS IRRELEVANCE RATHER THAN MALFORMEDNESS, and it has to
/// be: until the entry is joined to a name, nothing knows whether it is even a
/// room the operator listed, and a bridge that serves the whole house would
/// otherwise let a garbled entry in a room nobody watches refuse every poll.
fn watched_room(
    entry: &serde_json::Value,
    rooms: &[serde_json::Value],
    watched: &[String],
) -> Option<String> {
    let owner = entry.get("owner")?;
    // THE HOUSE ROLL-UP IS NOT A ROOM. `bridge_home` reports every sensor in
    // the building, so its edge is the newest edge anywhere and it would win
    // every comparison above while naming nowhere in particular.
    if owner.get("rtype")?.as_str()? != "room" {
        return None;
    }
    let room = room_name(rooms, owner.get("rid")?.as_str()?)?;
    watched.contains(&room).then_some(room)
}

/// One `motion_report` as the edge it states, or `None` for a report this
/// cannot read.
fn report_edge(report: &serde_json::Value, room: String) -> Option<EntryEdge> {
    let at = instant_from_utc(report.get("changed")?.as_str()?)?;
    Some(EntryEdge::Found {
        at,
        edge: Edge {
            // THE FRACTION IS DROPPED HERE AND ONLY HERE, because the state
            // file carries whole seconds. `at` keeps it for the comparison.
            epoch: at.0,
            motion: report.get("motion")?.as_bool()?,
            room,
        },
    })
}

/// The name of the room with this id, or `None` when the listing does not hold
/// it. A room renamed or removed between two polls simply stops matching,
/// which is a room that no longer reports rather than an error.
fn room_name(rooms: &[serde_json::Value], rid: &str) -> Option<String> {
    rooms
        .iter()
        .find(|room| room.get("id").and_then(serde_json::Value::as_str) == Some(rid))?
        .pointer("/metadata/name")?
        .as_str()
        .map(String::from)
}
