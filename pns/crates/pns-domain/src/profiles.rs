//! A profile: the named bundle of settings that decides what reaches the
//! operator. It changes DELIVERY only. The hooks still record every event and
//! the durable store still fills.
//!
//! IT ONLY EVER SUBTRACTS. The engine's own decision runs first and says where
//! the operator is and which surfaces an event would reach; a profile takes
//! surfaces away from that plan and can never add one, so no profile can card
//! a phone the operator is not near.

mod admission;
pub use admission::{Admits, Profile};
mod codec;
pub use codec::{format_override, parse_override};

/// The profile a machine with no rules matching is on, and the one a config
/// with no `[profiles]` table has.
pub const DEFAULT_PROFILE: &str = "default";

/// One `[[profiles.rules]]` row. Every input is optional; a rule naming none
/// matches always, and the FIRST rule whose named inputs all match wins.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Rule {
    pub profile: String,
    /// Weekdays, 0 for Sunday, as `libc` numbers them. Empty is not named.
    pub days: Vec<u32>,
    pub hours: Option<crate::lamps::QuietWindow>,
    pub location: Option<String>,
    pub focus: Option<String>,
    pub calendar_busy: Option<bool>,
}

/// The operator's own selection, which beats every rule while it stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Override {
    pub profile: String,
    /// The epoch second it ends at, or None for one that stands until cleared.
    pub until: Option<u64>,
}
