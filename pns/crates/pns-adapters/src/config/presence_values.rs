/// The `[plugins.presence]` settings this crate can act on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Presence {
    /// The bridge's own room names, verbatim, that a reading may name.
    pub rooms: Vec<String>,
    /// Rooms whose presence is discarded even when they are listed.
    pub exclude: Vec<String>,
    /// The room the desk is in, when the operator named one.
    pub desk_room: Option<String>,
    /// How long a desk reading still speaks for where the operator IS.
    pub desk_stale_after_secs: u64,
    pub poll_secs: u64,
    pub stale_after_secs: u64,
}

/// The only backend that fills the presence state file today.
pub(super) const PRESENCE_TYPE: &str = "hue";

/// How often the daemon reads the bridge, and the range it is held to. THE
/// FLOOR IS A COURTESY TO THE BRIDGE and the ceiling is the point at which a
/// reading is older than the turn it would narrow.
pub(super) const DEFAULT_PRESENCE_POLL_SECS: u64 = 5;

/// How long a desk reading still speaks for WHERE THE OPERATOR IS, as opposed
/// to where their attention is. THE SAME 120 SECONDS `engine`'s
/// `DEFAULT_DESK_IDLE_SECS` already trusts a desk reading inside, so the
/// shipped behaviour of the two clocks agrees; past it a keyboard nobody has
/// touched says nothing about which room somebody is standing in, and fresh
/// motion elsewhere wins. It is a knob because the right number is a property
/// of a house and a habit, not of this code.
pub(super) const DEFAULT_DESK_STALE_AFTER_SECS: u64 = 120;

/// The longest that knob may be turned. AN HOUR IS ALREADY THIRTY TIMES THE
/// SHIPPED VALUE, and the reading it bounds is "seconds since a keystroke": an
/// hour-old keystroke is a machine somebody walked away from, so anything past
/// this is not a house with slower habits, it is a typo. The bound exists
/// because the failure it prevents is silent and permanent, where a refusal is
/// one line the operator reads at the next apply.
pub(super) const MAX_DESK_STALE_AFTER_SECS: u64 = 3600;
pub(super) const MIN_PRESENCE_POLL_SECS: u64 = 2;
pub(super) const MAX_PRESENCE_POLL_SECS: u64 = 60;

/// How old a poll may be before there is no reading: three intervals at the
/// default, which rides out two missed polls without claiming a room nobody
/// refreshed.
pub(super) const DEFAULT_PRESENCE_STALE_AFTER_SECS: u64 = 15;
