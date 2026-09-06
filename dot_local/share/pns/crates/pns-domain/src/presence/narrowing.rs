//! What a room reading MEANS FOR THE LAMPS: the routing narrowed to the room
//! the operator is in, or left alone with a reason.
//!
//! ITS OWN MODULE, beside `presence` rather than inside it, for the split that
//! module's own doc draws: `presence` says what a READING means, and this says
//! what the lamps do about it.
//!
//! WHICH ROOM THE READINGS NAME IS `presence_room`'s QUESTION, not this one.
//! That module weighs the desk clock against the bridge's motion edge and
//! answers a room; this one takes that room to a lamp map. Its vocabulary,
//! `Snapshot` and `Full`, is re-exported here so a caller that only ever wants
//! the narrowing has one module to name.
//!
//! POLICY ONLY. Every function here is a total function of its arguments: no
//! bridge, no clock, no config file and no printing. The composition root
//! takes ONE snapshot of the world and hands it in.
//!
//! PRESENCE ONLY EVER NARROWS. Every way of not knowing leaves the routing
//! exactly as it was, and so does a narrowing that would leave no lamp at all:
//! silence is the one outcome this feature must never produce.

use crate::lamps::resolve::Routing;
use crate::presence::room::chosen;

pub use crate::presence::room::{Full, Snapshot};

/// What the narrowing did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Narrowing {
    /// Only the lamps the bridge places in this room were kept.
    To(String),
    /// The whole routing stands, and why.
    Full(Full),
}

/// The routing narrowed to the room the operator is in, and the decision that
/// says why.
pub fn narrow(mut routing: Routing, snapshot: &Snapshot) -> (Routing, Narrowing) {
    let room = match chosen(snapshot) {
        Ok(room) => room,
        Err(full) => return (routing, Narrowing::Full(full)),
    };
    // ASKED BEFORE ANYTHING IS DROPPED, so the whole routing is still here to
    // fall back to. Narrowing to a room the map routes nothing for would leave
    // the operator with no lamp at all and nothing said about why.
    if !routing.lamps.iter().any(|routed| holds(routed, &room)) {
        return (routing, Narrowing::Full(Full::NoLampIn(room)));
    }
    routing.lamps.retain(|routed| holds(routed, &room));
    (routing, Narrowing::To(room))
}

/// Whether one routed lamp belongs to a room.
///
/// THE BRIDGE'S OWN MEMBERSHIP, which `resolve` already joined off the room
/// listing and carried on every `Lamp`. A room derived from the lamp's NAME
/// instead would be a guess about a naming convention, and `resolve`'s own
/// rule is that the bridge's current membership is the truth: a lamp moved
/// between rooms answers its new room the moment the listing does.
fn holds(routed: &crate::lamps::resolve::Routed, room: &str) -> bool {
    routed.lamp.room.as_deref() == Some(room)
}

#[cfg(test)]
mod tests;
