//! The plain value types the lamp policy shares with the config edge that
//! parses them. No parsing lives here: a `[lights]` table is read at the edge
//! and arrives as these.

/// What a lamp can say. A CLOSED SET, which is the whole reason `[lights]` is
/// judged here instead of passed through as a plugin's free-form settings: a
/// `shows` list holding a word nothing matches is a lamp that stays dark while
/// the operator is sure they routed it, with no message anywhere.
///
/// `Unread` IS ONE WORD AND CARRIES TWO COLOURS. Its success and failure
/// flavours always ride the same lamp, so a config cannot route one without the
/// other and there is no spelling for trying.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Behaviour {
    Done,
    Failed,
    Blocked,
    Unread,
    Looping,
}
