//! The presence state file: what one line of it says, and nothing about what
//! it MEANS.
//!
//! SYNTAX ONLY, which is the whole reason this is not in `presence` beside the
//! policy that reads it. The two change for different reasons: the FORMAT
//! changes when the daemon that writes it changes, and the POLICY (how old a
//! reading may be, which rooms count) changes when the routing does. A parse
//! that also judged would be edited by both.

/// THE STATE FILE, owner-only, under the state directory, rewritten whole by
/// the daemon's poll and read here. Its name says what it carries; `type` in
/// `[plugins.presence]` names which backend filled it, and the file does not.
///
/// ONE LINE, in one of two shapes:
///
/// ```text
/// <poll_epoch>
/// <poll_epoch> <edge_epoch> <motion 0|1> <room>
/// ```
///
/// The first is a poll that completed and saw a motion edge in no configured
/// room, a real "nowhere" reading and deliberately distinct from no reading
/// at all. The second names the configured room with the newest edge, the
/// epoch of that edge, and whether motion is reported NOW.
///
/// THE POLL EPOCH IS WHAT MAKES A DEAD BRIDGE UNKNOWN. A failed poll writes
/// nothing, so the line ages out. Presence is never evidence that nobody is
/// home, only of where somebody already known to be here is.
///
/// THE ROOM IS THE BRIDGE'S OWN TEXT and crosses this parse VERBATIM: real
/// names carry spaces and dashes ("3F - Studio"), so the decision log's
/// identity filter would collapse every one of them. It is bounded and
/// refused for control characters here, and filtered where it is printed.
pub const STATE_FILE: &str = "presence";

/// The most of that file any reader pulls into memory. One line is under a
/// hundred bytes, so this is generous and still a bound: a file some other
/// hand grew is refused rather than allocated.
pub const READ_MAX: u64 = 4 * 1024;

/// The longest room name accepted, in characters. Longer is malformed rather
/// than truncated: a name this parse cut could never match a configured one.
const ROOM_MAX: usize = 64;

/// One line of the state file, as SYNTAX ALONE. Nothing here is aged or
/// matched against the config: `classify` is where a reading becomes a
/// verdict, so the two are read and tested apart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawPresence {
    pub poll_epoch: u64,
    /// `None` for the poll-only line: the poll ran and no configured room had
    /// an edge.
    pub edge: Option<Edge>,
}

/// The motion edge a poll found, and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    pub epoch: u64,
    /// Whether motion is reported NOW, rather than having been at `epoch`.
    pub motion: bool,
    /// The bridge's own room name, verbatim.
    pub room: String,
}

/// One line of the state file, or `None` when it is not one. A MISSING FILE
/// AND A MALFORMED LINE ARE ONE ANSWER: nothing a caller could do differs.
pub fn parse_presence_line(line: &str) -> Option<RawPresence> {
    let line = line.strip_suffix('\n').unwrap_or(line);
    let mut fields = line.splitn(4, ' ');
    let poll_epoch = crate::parse_count(fields.next()?)?;
    let Some(edge_epoch) = fields.next() else {
        return Some(RawPresence {
            poll_epoch,
            edge: None,
        });
    };
    let epoch = crate::parse_count(edge_epoch)?;
    let motion = match fields.next()? {
        "0" => false,
        "1" => true,
        _ => return None,
    };
    let room = fields.next()?;
    if room.is_empty() || room.chars().count() > ROOM_MAX || room.chars().any(char::is_control) {
        return None;
    }
    Some(RawPresence {
        poll_epoch,
        edge: Some(Edge {
            epoch,
            motion,
            room: room.to_string(),
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::{Edge, RawPresence, parse_presence_line};

    // --- parse_presence_line ------------------------------------------------

    #[test]
    fn a_full_line_carries_the_two_epochs_the_motion_flag_and_the_room() {
        assert_eq!(
            parse_presence_line("1700000100 1700000090 1 3F - Studio"),
            Some(RawPresence {
                poll_epoch: 1_700_000_100,
                edge: Some(Edge {
                    epoch: 1_700_000_090,
                    motion: true,
                    room: "3F - Studio".to_string(),
                }),
            })
        );
    }

    #[test]
    fn a_poll_only_line_is_a_reading_with_no_edge() {
        // A real answer, not the absence of one.
        assert_eq!(
            parse_presence_line("1700000100\n"),
            Some(RawPresence {
                poll_epoch: 1_700_000_100,
                edge: None,
            })
        );
    }

    #[test]
    fn a_malformed_line_is_no_reading_rather_than_a_partial_one() {
        for line in [
            "",
            "not-an-epoch",
            "1700000100 notanepoch 1 3F - Studio",
            "1700000100 1700000090 2 3F - Studio",
            "1700000100 1700000090 1 ",
            "1700000100 1700000090",
            "1700000100 1700000090 1",
        ] {
            assert_eq!(parse_presence_line(line), None, "{line:?} is malformed");
        }
    }

    #[test]
    fn a_room_name_carrying_a_control_character_is_malformed() {
        // The room crosses verbatim, so the one shape that could forge a
        // second line is refused here.
        assert_eq!(
            parse_presence_line("1700000100 1700000090 1 3F\nStudio"),
            None
        );
        assert_eq!(
            parse_presence_line("1700000100 1700000090 1 3F\u{1b}[2JStudio"),
            None
        );
    }

    #[test]
    fn a_room_name_past_the_bound_is_malformed_rather_than_truncated() {
        let long = "r".repeat(65);
        assert_eq!(
            parse_presence_line(&format!("1700000100 1700000090 1 {long}")),
            None
        );
        let at_the_bound = "r".repeat(64);
        assert!(
            parse_presence_line(&format!("1700000100 1700000090 1 {at_the_bound}")).is_some(),
            "the bound itself is still a room: only PAST it is malformed"
        );
    }
}
