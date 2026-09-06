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
