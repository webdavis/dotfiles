//! The decision log: why a card did or did not fire, in one line per event.
//!
//! POLICY ONLY, in `doctor.rs`'s style: every function here is a total
//! function of its arguments, with no config, no clock, no environment, no
//! file and no printing. The composition root reads the world, assembles a
//! `Record` out of values the decision ALREADY HAS, appends what comes back
//! and prints what the doctor asks for. This module never learns where the
//! file is.
//!
//! THE RECORD IS THE DECISION'S OWN READINGS, never a second reading taken
//! afterwards. That is why a `Record` is built from a `&Decision` rather than
//! from loose values: two readings of where the operator is can disagree, and
//! an explanation assembled from the later one describes a moment the decision
//! never saw. `engine` owns `GateInputs` for the same reason, and this module
//! depends on it rather than the other way round. The other three types it
//! names (`EventArgs`, `Leg`, `Delivery`) are the crate's own value types,
//! taken exactly as the composition root already holds them so that nothing is
//! transformed on the way in.

/// The record's VALUES moved to `pns-domain`: how many entries the section
/// keeps, the verdict per leg, the readings that may be absent, and the only
/// text a line carries. What stays here is the line's own shape and the
/// section that reads it back.
pub use pns_domain::KEPT;

pub use pns_domain::doctor::decision_section as section;

/// One decision, as everything needed to write its line. THE STRUCT IS THE
/// SCHEMA, and every field is a value the composition root already holds.
pub use pns_domain::Record;

#[cfg(test)]
mod tests;
