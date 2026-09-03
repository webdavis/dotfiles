//! The bridge side of the room sensor: two CLIP listings turned into one
//! reading for the state file.
//!
//! THE POLICY IS NOT HERE. `presence` decides what a reading means and
//! `presence_file` decides what a line looks like; this module decides only
//! which watched room moved last, which is the one question the bridge can
//! answer. The three change for three different reasons: a new backend, a new
//! rule, a new format.
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

use crate::channels::hue::Bridge;
use crate::presence_file::{Edge, RawPresence};

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
/// while a body it CAN read holding no watched edge is the poll-only reading,
/// which says the bridge answered and no watched room has reported. Collapsing
/// the two would let a garbled response claim the operator is nowhere.
pub fn reading(
    motion_json: &str,
    rooms_json: &str,
    watched: &[String],
    now: u64,
) -> Option<RawPresence> {
    let motion = data(motion_json)?;
    let rooms = data(rooms_json)?;
    Some(RawPresence {
        poll_epoch: now,
        // THE NEWEST EDGE AMONG THE WATCHED ROOMS, which is the only room this
        // can honestly name: an edge in a room nobody watches says nothing
        // about where the operator is, and letting one win would answer with a
        // room the config never listed.
        edge: motion
            .iter()
            .filter_map(|entry| edge_of(entry, &rooms, watched))
            .max_by_key(|edge| edge.epoch),
    })
}

/// The `.data[]` array of a CLIP response, or `None` for a body that has none.
///
/// ITS OWN COPY of `channels::hue`'s private helper, and deliberately not the
/// same function: that one answers an empty list for a body it could not read,
/// because a pulse with nothing to write is a no-op either way. Here the two
/// are different answers, and the difference is what a poll publishes.
fn data(clip_json: &str) -> Option<Vec<serde_json::Value>> {
    let body: serde_json::Value = serde_json::from_str(clip_json).ok()?;
    Some(body.get("data")?.as_array()?.clone())
}

/// One `grouped_motion` entry as an edge in a watched room, or `None` for
/// every entry that is not one.
fn edge_of(
    entry: &serde_json::Value,
    rooms: &[serde_json::Value],
    watched: &[String],
) -> Option<Edge> {
    let owner = entry.get("owner")?;
    // THE HOUSE ROLL-UP IS NOT A ROOM. `bridge_home` reports every sensor in
    // the building, so its edge is the newest edge anywhere and it would win
    // every comparison above while naming nowhere in particular.
    if owner.get("rtype")?.as_str()? != "room" {
        return None;
    }
    let room = room_name(rooms, owner.get("rid")?.as_str()?)?;
    if !watched.contains(&room) {
        return None;
    }
    // ABSENT FOR A ROOM WHOSE SENSORS ARE OFF, which serves `motion: {}`: no
    // report is no edge, never an edge at epoch zero.
    let report = entry.pointer("/motion/motion_report")?;
    Some(Edge {
        epoch: epoch_from_utc(report.get("changed")?.as_str()?)?,
        motion: report.get("motion")?.as_bool()?,
        room,
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

/// A CLIP instant (`2026-09-03T17:20:09.413Z`) as the second it names.
///
/// PURE ARITHMETIC, NOT A SYSTEM CALL, which is the one place this crate reads
/// a clock field without libc: turning a UTC civil time into an epoch second
/// involves no zone database, no daylight-saving transition and no leap
/// second. The other direction (`system::utc_timestamp`) asks libc because it
/// is handed an epoch and a zone question; this one is handed the answer.
///
/// STRICT ABOUT THE SHAPE, in `parse_count`'s spirit: a field this does not
/// recognise is `None`, so the room contributes no edge at all rather than an
/// edge at a second nobody meant. Milliseconds are DROPPED rather than
/// rounded, because the reading they feed is aged in whole seconds.
fn epoch_from_utc(stamp: &str) -> Option<u64> {
    if stamp.len() < 20 || !stamp.is_ascii() {
        return None;
    }
    let bytes = stamp.as_bytes();
    if [bytes[4], bytes[7], bytes[10], bytes[13], bytes[16]] != *b"--T::" {
        return None;
    }
    // THE FRACTION IS OPTIONAL AND EVERYTHING ELSE IS NOT: `Z` alone, or a
    // decimal point, digits and then `Z`. An offset (`+02:00`) is refused
    // rather than read as UTC, which would be an hour's error stated
    // confidently.
    let tail = &stamp[19..];
    let fraction = tail.strip_suffix('Z')?;
    if !(fraction.is_empty() || (fraction.starts_with('.') && digits(&fraction[1..]).is_some())) {
        return None;
    }
    let year = digits(&stamp[0..4])?;
    let month = digits(&stamp[5..7])?;
    let day = digits(&stamp[8..10])?;
    let hour = digits(&stamp[11..13])?;
    let minute = digits(&stamp[14..16])?;
    let second = digits(&stamp[17..19])?;
    // Range-checked before the arithmetic, so a garbled field is refused
    // rather than folded into a plausible-looking second.
    if !((1..=12).contains(&month)
        && (1..=31).contains(&day)
        && hour < 24
        && minute < 60
        && second < 61)
    {
        return None;
    }
    let seconds = days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second;
    // A PRE-EPOCH INSTANT IS REFUSED rather than wrapped: every reading this
    // feeds is aged against a Unix second.
    u64::try_from(seconds).ok()
}

/// Days from 1970-01-01 to this civil date, by Howard Hinnant's
/// `days_from_civil`. Signed on purpose: the shifted-era arithmetic runs
/// negative for every date before 1970-03-01, and the caller refuses the
/// result rather than wrapping it.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    // MARCH IS THE START OF THE YEAR here, which is what puts the leap day at
    // the END of it and lets one expression cover every year length.
    let year = year - i64::from(month <= 2);
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// A run of plain digits as a number, or `None`. `parse_count` cannot serve
/// here: it refuses a leading zero by design, and every field of a timestamp
/// is zero-padded.
fn digits(text: &str) -> Option<i64> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::{epoch_from_utc, poll, reading};
    use crate::channels::hue::Bridge;
    use crate::presence_file::{Edge, RawPresence};

    /// The `grouped_motion` body, in the live shape: the house roll-up, a room
    /// whose sensor is off, and two rooms with edges.
    const MOTION: &str = r#"{"data":[
        {"owner":{"rid":"kitchen","rtype":"room"},"enabled":true,"motion":{}},
        {"owner":{"rid":"studio","rtype":"room"},"enabled":true,
         "motion":{"motion_report":{"changed":"2026-09-03T17:20:09.413Z","motion":false}}},
        {"owner":{"rid":"hallway","rtype":"room"},"enabled":true,
         "motion":{"motion_report":{"changed":"2026-09-03T16:27:05.600Z","motion":false}}},
        {"owner":{"rid":"door","rtype":"room"},"enabled":true,
         "motion":{"motion_report":{"changed":"2026-09-03T17:30:00.000Z","motion":true}}},
        {"owner":{"rid":"house","rtype":"bridge_home"},"enabled":true,
         "motion":{"motion_report":{"changed":"2026-09-03T17:59:59.000Z","motion":true}}}
    ]}"#;

    /// The `room` listing that names those ids.
    const ROOMS: &str = r#"{"data":[
        {"id":"kitchen","metadata":{"name":"2F - Kitchen"}},
        {"id":"studio","metadata":{"name":"3F - Studio"}},
        {"id":"hallway","metadata":{"name":"3F - Hallway"}},
        {"id":"door","metadata":{"name":"1F - Front door"}},
        {"id":"house","metadata":{"name":"the house"}}
    ]}"#;

    fn watched() -> Vec<String> {
        vec!["3F - Studio".to_string(), "2F - Kitchen".to_string()]
    }

    // --- reading ------------------------------------------------------------

    #[test]
    fn the_newest_edge_among_the_watched_rooms_is_the_one_reported() {
        // TWO WATCHED ROOMS THAT BOTH REPORTED, which is what makes this about
        // newest rather than about "the only one there was": the hallway's
        // edge is 53 minutes older than the studio's.
        let watched = vec!["3F - Studio".to_string(), "3F - Hallway".to_string()];
        assert_eq!(
            reading(MOTION, ROOMS, &watched, 1_788_456_100),
            Some(RawPresence {
                poll_epoch: 1_788_456_100,
                edge: Some(Edge {
                    epoch: 1_788_456_009,
                    motion: false,
                    room: "3F - Studio".to_string(),
                }),
            })
        );
    }

    #[test]
    fn a_newer_edge_in_a_room_nobody_watches_never_displaces_a_watched_one() {
        // The front door is the newest ROOM edge in the body above, and it is
        // an arrival signal rather than a place the operator sits.
        let studio = reading(MOTION, ROOMS, &watched(), 1_788_456_100)
            .and_then(|raw| raw.edge)
            .expect("the studio edge");
        assert_eq!(studio.room, "3F - Studio");
    }

    #[test]
    fn the_house_roll_up_is_not_a_room() {
        // `bridge_home` holds the newest edge in the whole body, so a filter
        // that admitted it would win every comparison and name the house.
        let watched = vec!["the house".to_string()];
        assert_eq!(
            reading(MOTION, ROOMS, &watched, 1_788_456_100),
            Some(RawPresence {
                poll_epoch: 1_788_456_100,
                edge: None,
            })
        );
    }

    #[test]
    fn a_room_whose_sensor_is_switched_off_reports_no_edge() {
        // `motion: {}` is the live shape for it, and no report is no edge
        // rather than an edge at epoch zero.
        let watched = vec!["2F - Kitchen".to_string()];
        assert_eq!(
            reading(MOTION, ROOMS, &watched, 1_788_456_100),
            Some(RawPresence {
                poll_epoch: 1_788_456_100,
                edge: None,
            })
        );
    }

    #[test]
    fn a_watched_room_the_listing_does_not_name_reports_no_edge() {
        let watched = vec!["3F - Studio".to_string()];
        assert_eq!(
            reading(MOTION, r#"{"data":[]}"#, &watched, 1_788_456_100),
            Some(RawPresence {
                poll_epoch: 1_788_456_100,
                edge: None,
            })
        );
    }

    #[test]
    fn a_body_this_cannot_read_is_no_poll_at_all_rather_than_a_nowhere() {
        // The difference is what gets published: no reading writes nothing and
        // ages out to Unknown, where a "nowhere" would be a claim.
        for (motion, rooms) in [
            ("not json", ROOMS),
            ("{}", ROOMS),
            (r#"{"data":{}}"#, ROOMS),
            (r#"{"errors":[{"description":"unauthorised"}]}"#, ROOMS),
            (MOTION, "not json"),
            (MOTION, r#"{"rooms":[]}"#),
        ] {
            assert_eq!(
                reading(motion, rooms, &watched(), 1_788_456_100),
                None,
                "{motion:.20} with {rooms:.20} answered anyway"
            );
        }
    }

    #[test]
    fn a_body_with_an_empty_data_array_is_a_poll_that_saw_nothing() {
        assert_eq!(
            reading(r#"{"data":[]}"#, ROOMS, &watched(), 1_788_456_100),
            Some(RawPresence {
                poll_epoch: 1_788_456_100,
                edge: None,
            })
        );
    }

    // --- poll ---------------------------------------------------------------

    /// A bridge that answers each path from a table, and `None` for anything
    /// it was not given.
    struct ScriptedBridge(Vec<(&'static str, &'static str)>);

    impl Bridge for ScriptedBridge {
        fn get(&self, path: &str) -> Option<String> {
            self.0
                .iter()
                .find(|(served, _)| *served == path)
                .map(|(_, body)| (*body).to_string())
        }

        fn put(&self, _path: &str, _body: &str) {
            unreachable!("the poll never writes to the bridge");
        }
    }

    #[test]
    fn a_poll_reads_both_listings_and_names_the_room() {
        let bridge = ScriptedBridge(vec![("grouped_motion", MOTION), ("room", ROOMS)]);
        assert_eq!(
            poll(&bridge, &watched(), 1_788_456_100)
                .and_then(|raw| raw.edge)
                .map(|edge| edge.room),
            Some("3F - Studio".to_string())
        );
    }

    #[test]
    fn a_bridge_that_answers_neither_listing_answers_no_reading() {
        for served in [
            vec![],
            vec![("grouped_motion", MOTION)],
            vec![("room", ROOMS)],
        ] {
            assert_eq!(
                poll(&ScriptedBridge(served), &watched(), 1_788_456_100),
                None
            );
        }
    }

    // --- epoch_from_utc -----------------------------------------------------

    #[test]
    fn an_instant_becomes_the_second_it_names_and_its_milliseconds_are_dropped() {
        assert_eq!(epoch_from_utc("2026-09-03T17:20:09Z"), Some(1_788_456_009));
        assert_eq!(
            epoch_from_utc("2026-09-03T17:20:09.413Z"),
            Some(1_788_456_009)
        );
        assert_eq!(epoch_from_utc("1970-01-01T00:00:00Z"), Some(0));
        // A leap day and the day the shifted-era arithmetic starts its year.
        assert_eq!(epoch_from_utc("2024-02-29T12:34:56Z"), Some(1_709_210_096));
        assert_eq!(epoch_from_utc("2000-03-01T00:00:00Z"), Some(951_868_800));
    }

    #[test]
    fn an_instant_this_does_not_recognise_contributes_no_edge() {
        for stamp in [
            "",
            "2026-09-03",
            "2026-09-03T17:20:09",
            "2026-09-03 17:20:09Z",
            "2026-09-03T17:20:09+02:00",
            "2026-13-03T17:20:09Z",
            "2026-09-32T17:20:09Z",
            "2026-09-03T24:20:09Z",
            "2026-09-03T17:60:09Z",
            "20xx-09-03T17:20:09Z",
            "1969-12-31T23:59:59Z",
            "2026-09-03T17:20:09.Z",
            "2026-09-03T17:20:09.413",
        ] {
            assert_eq!(epoch_from_utc(stamp), None, "{stamp:?} was accepted");
        }
    }
}
