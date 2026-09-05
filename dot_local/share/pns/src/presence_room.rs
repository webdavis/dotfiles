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

use crate::home::HomePresence;
use crate::presence::{PresenceStatus, Unreadable};

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
    /// The desk is the freshest thing there is and no `desk_room` says which
    /// room it is in.
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
mod tests {
    use super::{Full, Snapshot, chosen};
    use crate::home::{DeviceKey, HomePresence};
    use crate::presence::{PresenceStatus, Unreadable};

    /// The operator's own snapshot, with the desk cold and the phone home.
    fn snapshot(status: PresenceStatus) -> Snapshot {
        Snapshot {
            status,
            desk_idle_secs: None,
            screen_locked: Some(false),
            home: HomePresence::Home {
                matched_by: DeviceKey::Hostname,
                value: "mister".to_string(),
            },
            desk_room: Some("3F - Studio".to_string()),
            desk_stale_after_secs: 120,
            now: Some(1_700_000_000),
        }
    }

    /// A fresh reading naming one room.
    fn in_room(room: &str) -> PresenceStatus {
        PresenceStatus::Room {
            room: room.to_string(),
            age_secs: 0,
        }
    }

    /// The study, as the answer this module returns.
    fn studio() -> Result<String, Full> {
        Ok("3F - Studio".to_string())
    }

    #[test]
    fn a_fresh_reading_with_no_desk_to_weigh_it_against_names_its_own_room() {
        assert_eq!(
            chosen(&snapshot(in_room("2F - Kitchen"))),
            Ok("2F - Kitchen".to_string())
        );
    }

    #[test]
    fn a_poll_that_found_nobody_answers_nothing_and_keeps_its_own_reason() {
        assert_eq!(
            chosen(&snapshot(PresenceStatus::Nowhere { poll_age_secs: 3 })),
            Err(Full::Nowhere)
        );
    }

    #[test]
    fn every_unreadable_reading_answers_nothing_and_keeps_its_own_reason() {
        // FIVE DIFFERENT THINGS TO GO AND FIX, and the variant is what carries
        // which: collapsed to a single "unknown" the operator cannot tell a
        // daemon that stopped from a room this config never watches. The five
        // WORDINGS are pinned where they are written down, in
        // `presence_journal::every_way_a_routing_can_be_left_whole_names_its_own_reason`.
        for reason in [
            Unreadable::NoReading,
            Unreadable::NoClock,
            Unreadable::Stale { poll_age_secs: 90 },
            Unreadable::Future,
            Unreadable::NotWatched,
        ] {
            assert_eq!(
                chosen(&snapshot(PresenceStatus::Unknown(reason))),
                Err(Full::Unknown(reason)),
                "{reason:?}"
            );
        }
    }

    #[test]
    fn a_desk_still_being_typed_at_beats_motion_of_the_same_age() {
        // A cat crossing the kitchen must not move the lamps off a keyboard
        // somebody is typing on: nobody is in two rooms at once, and only one
        // of these two readings is made by a human hand.
        let taken = Snapshot {
            desk_idle_secs: Some(0),
            ..snapshot(in_room("2F - Kitchen"))
        };
        assert_eq!(chosen(&taken), studio());
    }

    #[test]
    fn a_warm_desk_and_newer_motion_in_another_room_answer_nothing_at_all() {
        // THE BRIDGE REPORTS A STILL-OCCUPIED ROOM AS AGE ZERO, so three
        // seconds after a keystroke the kitchen is "newer" by the clock while
        // the operator is still at the keyboard. Two live readings that cannot
        // both be them, and nothing here can say which is: the whole house
        // signals rather than the wrong half of it.
        for desk_idle_secs in [1, 3, 60, 119] {
            let taken = Snapshot {
                desk_idle_secs: Some(desk_idle_secs),
                ..snapshot(in_room("2F - Kitchen"))
            };
            assert_eq!(
                chosen(&taken),
                Err(Full::Ambiguous {
                    desk: "3F - Studio".to_string(),
                    motion: "2F - Kitchen".to_string(),
                }),
                "at {desk_idle_secs}s idle the kitchen edge is somebody, and not provably them"
            );
        }
    }

    #[test]
    fn a_warm_desk_and_newer_motion_in_its_own_room_agree_and_answer_that_room() {
        // The two readings are not in conflict at all: somebody moved in the
        // room the keyboard is in. Left ambiguous, the one case where both
        // sensors say the same thing would narrow nothing.
        let taken = Snapshot {
            desk_idle_secs: Some(3),
            ..snapshot(in_room("3F - Studio"))
        };
        assert_eq!(chosen(&taken), studio());
    }

    #[test]
    fn a_desk_past_its_bound_stops_competing_and_motion_answers_alone() {
        // Past the bound the keyboard says nothing about which room a body is
        // standing in, so there are no longer two readings to weigh.
        for desk_idle_secs in [120, 121, 300] {
            let taken = Snapshot {
                desk_idle_secs: Some(desk_idle_secs),
                ..snapshot(in_room("2F - Kitchen"))
            };
            assert_eq!(
                chosen(&taken),
                Ok("2F - Kitchen".to_string()),
                "at {desk_idle_secs}s idle the desk has no claim left"
            );
        }
    }

    #[test]
    fn a_desk_nobody_has_touched_for_longer_than_the_bound_speaks_for_nothing() {
        // Past the bound the desk stops competing at all, so a reading that
        // says only "nowhere" no longer parks every signal in the study.
        let cold = Snapshot {
            desk_idle_secs: Some(120),
            ..snapshot(PresenceStatus::Nowhere { poll_age_secs: 3 })
        };
        assert_eq!(chosen(&cold), Err(Full::Nowhere));
        // AND ONE SECOND UNDER IT STILL DOES, or the bound is one short and
        // nobody could tell from the outside.
        let warm = Snapshot {
            desk_idle_secs: Some(119),
            ..cold
        };
        assert_eq!(chosen(&warm), studio());
    }

    #[test]
    fn a_locked_screen_disqualifies_the_desk_however_recent_its_last_keystroke() {
        // Locking necessarily postdates the last keystroke, so it is the
        // newest fact about the desk rather than an exception to the rule.
        let taken = Snapshot {
            desk_idle_secs: Some(0),
            screen_locked: Some(true),
            ..snapshot(in_room("2F - Kitchen"))
        };
        assert_eq!(chosen(&taken), Ok("2F - Kitchen".to_string()));
    }

    #[test]
    fn a_desk_reading_nobody_could_take_never_competes() {
        // `None` must never coerce to zero, which would read as actively
        // typing and park every signal in the study for good.
        let taken = Snapshot {
            desk_idle_secs: None,
            ..snapshot(in_room("2F - Kitchen"))
        };
        assert_eq!(chosen(&taken), Ok("2F - Kitchen".to_string()));
    }

    #[test]
    fn a_warm_desk_with_no_room_named_for_it_answers_nothing_rather_than_guessing() {
        let taken = Snapshot {
            desk_idle_secs: Some(0),
            desk_room: None,
            ..snapshot(PresenceStatus::Nowhere { poll_age_secs: 3 })
        };
        assert_eq!(chosen(&taken), Err(Full::NoDeskRoom));
    }

    #[test]
    fn a_phone_off_the_home_network_answers_nothing_however_fresh_the_motion_is() {
        // Somebody is moving in the kitchen and it is not the operator. The
        // house is not theirs to narrow, so the whole routing stands.
        let taken = Snapshot {
            home: HomePresence::NotHome,
            ..snapshot(in_room("2F - Kitchen"))
        };
        assert_eq!(chosen(&taken), Err(Full::NotHome));
    }

    #[test]
    fn a_router_that_could_not_answer_still_lets_motion_carry_the_lamps() {
        // Fresh motion in a watched room is itself evidence of a human in that
        // room, and it is better evidence than a router nobody could reach.
        // Read the other way, a machine with no router table would lose the
        // whole feature.
        let taken = Snapshot {
            home: HomePresence::Unknown,
            ..snapshot(in_room("2F - Kitchen"))
        };
        assert_eq!(chosen(&taken), Ok("2F - Kitchen".to_string()));
    }

    #[test]
    fn a_desk_being_typed_at_outranks_a_router_that_says_nobody_is_home() {
        // The keyboard is the operator's own hand. A router that disagrees
        // with it is wrong about the router, not about the desk.
        let taken = Snapshot {
            desk_idle_secs: Some(0),
            home: HomePresence::NotHome,
            ..snapshot(in_room("2F - Kitchen"))
        };
        assert_eq!(chosen(&taken), studio());
    }
}
