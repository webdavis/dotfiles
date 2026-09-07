//! The bridge side of the room sensor: two CLIP listings turned into one
//! reading for the state file.
//!
//! THE POLICY IS NOT HERE. `presence` decides what a reading means,
//! `presence_file` decides what a line looks like and `presence_instant`
//! decides what an instant is; this module decides only which watched room
//! moved last, which is the one question the bridge can answer. The four
//! change for four different reasons: a new backend, a new rule, a new
//! format, a new field shape.
//!
//! WHAT THE BRIDGE ACTUALLY SERVES, verified live on 2026-09-03 and the reason
//! the shapes below are refused the way they are: `grouped_motion` carries one
//! entry per room PLUS one owned by `bridge_home`, which is the whole house and
//! never a room; a room whose only sensor is switched off carries `motion: {}`
//! with no report inside it; and the `changed` instant carries MILLISECONDS
//! (`2026-09-03T17:20:09.413Z`). A motion body names no room, so the room name
//! is joined through the `room` listing by `owner.rid`, never by a name inside
//! the motion body: there is none.
//!
//! OPEN FACT, and the one thing here nobody can settle yet: the machine has
//! ZERO MotionAware areas, so whether an area's motion joins its room's
//! `grouped_motion` roll-up or arrives only as `convenience_area_motion` owned
//! by a `motion_area_configuration` is unverifiable. This reads the roll-up,
//! which is the shape that exists. See `docs/specs/daemon-jobs.md` for the one
//! GET that settles it once an area exists.

use super::state_file::{Edge, RawPresence};
mod parse;
use crate::hue::Bridge;
use parse::{EntryEdge, data, edge_of};

/// One poll: both listings, and the reading they make.
///
/// `None` IS A BRIDGE THAT DID NOT ANSWER, and the caller must publish nothing
/// for it. That is the whole fail-closed guarantee: a line that stops arriving
/// ages out to Unknown, where a line written anyway would pin the operator in
/// a room, or out of every room, on the word of a bridge that said nothing.
///
/// BOTH LISTINGS OR NOTHING, in `resolve_on_bridge`'s style: the motion body
/// carries rids and the room body carries the names they mean, so a poll
/// holding one of the two knows that something moved and not where.
pub fn poll<B: Bridge>(bridge: &B, watched: &[String], now: u64) -> Option<RawPresence> {
    let motion = bridge.get("grouped_motion")?;
    let rooms = bridge.get("room")?;
    reading(&motion, &rooms, watched, now)
}

/// What the two bodies say, as one reading. Pure, so the whole of the parse is
/// testable against bodies copied off the live bridge.
///
/// A BODY THIS CANNOT READ IS NOT AN ANSWER (`None`, so nothing is published),
/// and neither is a body carrying a WATCHED room whose report it cannot read,
/// while a body it CAN read holding no watched edge is the poll-only reading,
/// which says the bridge answered and no watched room has reported. Collapsing
/// those would let a garbled response claim the operator is nowhere.
pub fn reading(
    motion_json: &str,
    rooms_json: &str,
    watched: &[String],
    now: u64,
) -> Option<RawPresence> {
    let motion = data(motion_json)?;
    let rooms = data(rooms_json)?;
    // THE NEWEST EDGE AMONG THE WATCHED ROOMS, which is the only room this can
    // honestly name: an edge in a room nobody watches says nothing about where
    // the operator is, and letting one win would answer with a room the config
    // never listed.
    //
    // AND NO EDGE AT ALL WHEN ONE OF THEM IS UNREADABLE, which is why this is
    // a loop rather than a `filter_map`: the room whose report would not parse
    // may hold the newer edge, so the newest of the rest is a guess.
    //
    // COMPARED AT FULL PRECISION AND PUBLISHED IN WHOLE SECONDS. The bridge's
    // `changed` carries milliseconds, so reducing to the state file's format
    // BEFORE the comparison made two edges inside one second compare equal and
    // handed the answer to the order the bridge listed its rooms in.
    //
    // AND ORDERED TOTALLY, so an exact tie is decided by the reading and never
    // by the response. Two sensors reporting inside one millisecond is a
    // coincidence rather than an impossibility, and the bridge's listing order
    // is not stable, so a comparison that let the later ENTRY win answered a
    // different room from one poll to the next with nothing having changed.
    // The room name breaks the tie and the motion flag breaks that, which is
    // arbitrary on purpose: what matters is that it is a fact about the two
    // edges rather than about the order they arrived in.
    let mut newest: Option<((u64, u32), Edge)> = None;
    for entry in &motion {
        match edge_of(entry, &rooms, watched) {
            EntryEdge::Malformed => return None,
            EntryEdge::Found { at, edge } => {
                if newest.as_ref().is_none_or(|(held_at, held)| {
                    (*held_at, &held.room, held.motion) < (at, &edge.room, edge.motion)
                }) {
                    newest = Some((at, edge));
                }
            }
            EntryEdge::Irrelevant => {}
        }
    }
    Some(RawPresence {
        poll_epoch: now,
        edge: newest.map(|(_, edge)| edge),
    })
}

/// The join's own tests.
#[cfg(test)]
mod tests;

/// The selection's own tests beside them: which entries are evidence at all,
/// and the order two that are get compared in.
#[cfg(test)]
mod selection_tests;
