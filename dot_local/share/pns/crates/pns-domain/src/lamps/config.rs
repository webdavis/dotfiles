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

/// A breath: how long ONE fade takes, and the two ends it fades between.
///
/// `high` IS THE PEAK. The held record tracks which end a breath last landed
/// on (`resume_from` in `lights.rs`), and every fade the driver issues moves
/// toward one of these two named values, which is why `low` above `high` is
/// refused at load: with the ends reversed, a fade to `high` would move the
/// lamp DOWN and one to `low` would move it up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Breath {
    pub duration_ms: u64,
    pub high: u8,
    pub low: u8,
}

/// The loop lamp's motion: a breath with an accent at its peak.
///
/// THE LOOP'S OWN SHAPE TYPE, and not two more fields on `Breath`, for the
/// config ruling stated at `Lights`: only the knobs that APPLY to a behaviour
/// exist. `Breath` is what the blocked lamp, both unread lamps and the shared
/// dim form run, none of which flare, so an accent parked on `Breath` would be
/// four dead knobs on three behaviours for a reader to set and watch do
/// nothing.
///
/// IT COMPOSES A `Breath` RATHER THAN RESTATING ONE. The two fades either side
/// of the accent are an ordinary breath and are parsed, bounded and checked by
/// the same arm every other breathing shape uses; the accent is the only thing
/// this type adds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BreatheThenFlare {
    pub breath: Breath,
    /// The brightness the accent reaches, above the breath's own `high`.
    pub flare: u8,
    /// How long the accent takes, which is what makes it a flash rather than a
    /// third fade.
    pub flare_ms: u64,
}
