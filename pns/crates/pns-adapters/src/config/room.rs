/// The longest room name accepted, in characters. Longer is malformed rather
/// than truncated: a name this parse cut could never match a configured one.
pub const ROOM_MAX: usize = 64;

/// Whether this file can carry that room name verbatim.
///
/// PUBLIC BECAUSE THE CONFIG READS IT TOO. A room is the bridge's own text by
/// way of the operator's config, and one this refuses renders no line at all,
/// which is the right answer for the write and a silent one for the operator:
/// the poll publishes nothing and the doctor can only report a reading that is
/// stale or absent. Asking the same question at the config edge turns it into
/// a named configuration error. One predicate rather than two, so the answer
/// cannot drift between the file that writes the line and the table that
/// names the room.
pub fn room_fits(room: &str) -> bool {
    !room.is_empty() && room.chars().count() <= ROOM_MAX && !room.chars().any(char::is_control)
}
