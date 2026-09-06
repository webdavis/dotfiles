//! WHICH ROOM THE READINGS PUT THE OPERATOR IN, and why none of them does.
//!
//! ITS OWN MODULE beside `presence_policy`, for the split that module's own
//! doc draws one step further out: `presence` says what a READING means, this
//! says WHERE THE BODY IS, and `presence_policy` says what the lamps do about
//! it. The arbitration between the desk clock and the bridge's motion edge is
//! a question about two sensors and knows nothing about a lamp; the narrowing
//! is a question about a lamp map and knows nothing about a keyboard.
//!
//! IT DOES NOT READ THE NOTIFICATION SURFACE, and that is the correction this
//! module exists to hold. `surface::Surface` answers WHERE THE OPERATOR'S EYES
//! ARE, for picking a notifier; it is `Desk` for two minutes after the last
//! keystroke and `Away` whenever neither the keyboard nor the phone has been
//! touched lately. Neither is a claim about which room a body is standing in:
//! read as location, `Desk` ignores fresh motion in the kitchen for two
//! minutes after the operator walks out of the study, and `Away` reads a phone
//! sitting in a pocket at home as an empty house. The inputs here are physical
//! instead: the desk's own idle clock, the bridge's motion edge, and the
//! router's answer about whether the phone is on the home network.
//!
//! POLICY ONLY. Every function here is a total function of its arguments: no
//! bridge, no clock, no config file and no printing. The composition root
//! takes ONE snapshot of the world and hands it in.

use crate::home::reading::HomePresence;
use crate::presence::status::{PresenceStatus, Unreadable};

/// Everything the narrowing is a function of, taken at ONE moment.
///
/// ONE STRUCT AND NOT SIX ARGUMENTS, for the reason `engine`'s own
/// `SurfaceReading` is one: these are one judgement over one set of readings,
/// and a caller free to take any of them again further down is a caller free
/// to take it at a different moment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    /// What the bridge's motion roll-up says, already aged and judged.
    pub status: PresenceStatus,
    /// Seconds since the desk keyboard was touched. `None` is a reading
    /// nobody could take, which is never the same as zero.
    pub desk_idle_secs: Option<u64>,
    /// The desk display's lock. ONLY `Some(true)` DISQUALIFIES THE DESK, which
    /// is `surface::surface`'s own rule and is newest-signal-wins rather than
    /// an exception to it: locking necessarily postdates the last keystroke.
    pub screen_locked: Option<bool>,
    /// Whether the phone is on the home network.
    pub home: HomePresence,
    /// The room the desk is in, when the operator named one.
    pub desk_room: Option<String>,
    /// How long a desk reading still speaks for where the operator IS.
    pub desk_stale_after_secs: u64,
    /// The clock every age above was aged against, carried so the record is
    /// stamped with the moment the readings were taken.
    pub now: Option<u64>,
}

/// Why a routing was left whole. EVERY VARIANT IS A DIFFERENT THING TO GO AND
/// FIX, which is what an `Option` here could not have carried.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Full {
    /// The router says the phone is not on the home network, so the motion is
    /// somebody or something else.
    NotHome,
    /// The desk still speaks for where the operator is, no `desk_room` says
    /// which room that is, AND NOTHING ELSE COULD ANSWER EITHER. With a
    /// usable motion edge to fall back on, that edge answers instead and this
    /// is never reached: an unnamed desk room costs the desk its vote, not
    /// the narrowing.
    NoDeskRoom,
    /// A desk still inside its bound in one room, and newer motion in another.
    /// TWO LIVE READINGS THAT CANNOT BOTH BE THE OPERATOR, and nothing here
    /// can say which one is.
    Ambiguous { desk: String, motion: String },
    /// A fresh poll found motion in no watched room. NOT a claim that nobody
    /// is home: the room they are in may have no sensor.
    Nowhere,
    /// No usable reading, and which kind.
    Unknown(Unreadable),
    /// The room holds no routed lamp, so narrowing to it would light nothing.
    NoLampIn(String),
}

/// Which room the readings put the operator in, or why none of them does.
///
/// A WARM DESK IS A CLAIM ON THE OPERATOR'S BODY, and the whole arbitration
/// falls out of that. Inside `desk_stale_after_secs` the keyboard says the
/// operator is at the desk; motion says A BODY moved in a room, and never
/// whose. So while the desk still speaks:
///
/// - MOTION IN THE DESK'S OWN ROOM AGREES with it, and the room is kept.
/// - MOTION NO NEWER THAN THE DESK loses to it. THE TIE GOES TO THE DESK,
///   where a hand is what made the reading: nobody is in two rooms at once,
///   and a cat crossing the kitchen must not move the lamps off a keyboard
///   being typed on.
/// - NEWER MOTION SOMEWHERE ELSE IS AMBIGUOUS AND NARROWS NOTHING. A bridge
///   reports a room that is STILL occupied as age zero, not as the age of the
///   edge that began it, so "newer" here is routinely three seconds after a
///   keystroke and means only that somebody is in that other room. Read as
///   the operator having walked out, it hands every lamp to whoever else is
///   moving in the house while the operator is still typing.
///
/// PAST THE BOUND, LOCKED, OR UNREADABLE, the desk has no claim at all and
/// motion answers alone.
pub fn chosen(snapshot: &Snapshot) -> Result<String, Full> {
    let motion = match &snapshot.status {
        PresenceStatus::Room { room, age_secs } => Ok((room, *age_secs)),
        PresenceStatus::Nowhere { .. } => Err(Full::Nowhere),
        PresenceStatus::Unknown(reason) => Err(Full::Unknown(*reason)),
    };
    let desk_age = desk_age(snapshot);
    if let (Some(desk), Some(desk_room)) = (desk_age, snapshot.desk_room.as_ref()) {
        return match motion {
            Ok((room, motion_age)) if room == desk_room || desk <= motion_age => {
                Ok(desk_room.clone())
            }
            Ok((room, _)) => Err(Full::Ambiguous {
                desk: desk_room.clone(),
                motion: room.clone(),
            }),
            Err(_) => Ok(desk_room.clone()),
        };
    }
    let (room, _) = match motion {
        Ok(found) => found,
        // A desk that would have answered with no room named for it is a
        // different thing to go and fix from a poll that found nobody, and it
        // is only worth saying when nothing else could have answered either.
        Err(full) => {
            return Err(if desk_age.is_some() {
                Full::NoDeskRoom
            } else {
                full
            });
        }
    };
    // THE ROUTER IS ASKED ABOUT MOTION AND NEVER ABOUT THE DESK. A keyboard
    // being typed on is the operator's own hand, so a router that says nobody
    // is home while the desk is warm is wrong about the router. Motion has no
    // such author: it is a body, and the router is what says whose.
    //
    // ONLY `NotHome` GATES. `Unknown` is a router nobody could reach, or a
    // machine that never armed one, and read as absence it would take the
    // whole feature away; fresh motion in a watched room is itself evidence of
    // a human in that room.
    //
    // WHICH MAKES THIS GATE DORMANT IN PRODUCTION TODAY, and that is accepted
    // rather than overlooked. Nothing publishes a home reading yet, so both
    // callers hand in `Unknown` and only a test ever reaches the refusal
    // below. The publisher is filed as B102, and until it lands an operator
    // out of the house with somebody else moving in the kitchen narrows the
    // lamps to the kitchen.
    //
    // THE COST OF THAT IS BOUNDED BY WHAT PRESENCE IS. It only ever narrows
    // which lamp signals, and every room is empty of the operator when they
    // are away, so a wrong narrow costs them nothing a full write would have
    // delivered either: they are not in the house to see any of it. The
    // signal that actually reaches them when they are away is the phone, and
    // no lamp decision touches that leg. Requiring `Home` instead would trade
    // this dormant gate for a live one that switches the whole feature off on
    // every machine with no router table at all, which is the failure
    // direction the paragraph above rules out.
    if snapshot.home == HomePresence::NotHome {
        return Err(Full::NotHome);
    }
    Ok(room.clone())
}

/// How long since the desk was touched, when that reading still speaks for
/// where the operator IS.
///
/// THREE WAYS IT SPEAKS FOR NOTHING, and none of them may become a zero:
/// unreadable (never the same as "actively typing"), a locked screen (which
/// necessarily postdates the last keystroke, so it is the NEWEST fact about
/// the desk), and older than the bound the operator set.
fn desk_age(snapshot: &Snapshot) -> Option<u64> {
    snapshot
        .desk_idle_secs
        .filter(|_| snapshot.screen_locked != Some(true))
        .filter(|age| *age < snapshot.desk_stale_after_secs)
}

#[cfg(test)]
mod tests;
