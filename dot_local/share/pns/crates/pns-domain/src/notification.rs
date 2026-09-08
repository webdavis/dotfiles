//! The event a producer raised, as the value every destination is handed.
//!
//! THE WIRE FORMAT IS NOT HERE. How this becomes the channel contract's JSON
//! object is the executable destination's business and lives with it: the two
//! change for different reasons, the FORMAT when the contract does and this
//! struct when a producer starts carrying a new field.

/// One rendered event, the structured form of the channel contract's JSON
/// object. The pane is the SANITIZED one.
#[derive(Debug, Default, PartialEq)]
pub struct Event {
    pub agent: String,
    pub state: String,
    pub project: String,
    pub branch: String,
    pub detail: String,
    pub title: String,
    pub message: String,
    pub preview: String,
    pub pane: String,
}

/// The parsed event arguments. Every field defaults to empty or false, so a
/// bare invocation is valid and renders an empty event.
#[derive(Debug, Default, PartialEq)]
pub struct EventArgs {
    pub agent: String,
    pub state: String,
    pub project: String,
    pub branch: String,
    pub detail: String,
    pub pane: String,
    /// The named hermes route this event posts to, resolved through the
    /// config's `[plugins.hermes]` channels table; empty means the default
    /// (alert) route. Names, not URLs: the caller says WHERE, the config
    /// says HOW to get there.
    pub channel: String,
    pub local_only: bool,
    pub remote_only: bool,
    /// The >=300s tier: the lights signal rides on top of whatever else the
    /// plan decides.
    pub long_running: bool,
}
