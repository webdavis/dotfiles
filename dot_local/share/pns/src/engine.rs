//! The engine: one event in, a delivery plan out, every decision delegated.
//!
//! This module ORCHESTRATES the decision core against the probe seams; it
//! owns no policy of its own. Two properties are load-bearing and pinned by
//! recording probes rather than by outcomes alone:
//!
//! PROBES RUN ONLY WHEN THEIR ANSWER COULD MATTER. Every reading is a spawn
//! on a path that must never stall, so a caller who already stated an answer
//! never pays for the probe underneath it: an idle override skips the idle
//! read and the screen-lock read that only exists to qualify it, and a stated
//! phone-input age skips the process walk behind it.
//!
//! CALLER INTENT IS NEVER OVERRIDDEN. Skip beats force ("I already sent it"
//! is more specific than an override), the narrowing flags beat both, and
//! force exempts the event from viewed-pane suppression.

// THE DECISION'S VALUE TYPES moved to `pns-domain`, and the decision itself
// followed the probe traits to `pns-application` once those became ports.
// Nothing is left of this module but the paths its callers already name.
pub use pns_domain::decision::{
    DEFAULT_DESK_IDLE_SECS, Decision, GateInputs, Overrides, SurfaceReading,
};

/// THE DECISION ITSELF moved to `pns-application`, once the probe traits it
/// is generic over became ports there. This is the path its callers name.
pub use pns_application::decide::decide;

/// THE ENVIRONMENT READING moved to `pns-application`, where the probe ports
/// it is generic over are declared.
pub use pns_application::environment_reading::{
    operator_surface, operator_visibility, surface_reading,
};

#[cfg(test)]
mod fixtures;

#[cfg(test)]
mod readings_tests;

#[cfg(test)]
mod plan_tests;

#[cfg(test)]
mod mute_tests;

#[cfg(test)]
mod intent_tests;

#[cfg(test)]
mod guard_tests;
