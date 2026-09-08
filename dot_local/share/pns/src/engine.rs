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

// The decision values live in `pns-domain`. PR 6.1 separates acquisition into
// application and pure arbitration over a completed typed snapshot into domain.
// Keep this legacy call surface until composition replaces it.
pub use pns_domain::{DEFAULT_DESK_IDLE_SECS, Decision, GateInputs, Overrides, SurfaceReading};

use crate::probes::{
    IdleProbe, PhoneInputProbe, PhoneMarkerProbe, ProbeStart, ScreenLockProbe, SessionViewProbe,
};
use crate::registry::Selection;

/// Decide the plan for one event. `now_secs` is the wall clock, taken once at
/// the edge; `None` reads as an unreadable clock, which ages nothing.
///
/// ASSEMBLY ONLY. Where the operator is looking is `surface::surface`, whether
/// the origin pane is on screen is `surface::visibility`, and what to do about
/// it is `surface::plan`. This reads the probes those three need and turns the
/// plan into legs.
// Preserve the existing external call while composition migrates to DecisionRequest.
#[allow(clippy::too_many_arguments)]
pub fn decide<P>(
    probes: &P,
    selection: &Selection,
    overrides: &Overrides,
    local_only: bool,
    remote_only: bool,
    pane: &str,
    now_secs: Option<u64>,
    long_running: bool,
    mobile_watch_card: bool,
) -> Decision
where
    P: IdleProbe
        + PhoneMarkerProbe
        + PhoneInputProbe
        + ScreenLockProbe
        + SessionViewProbe
        + ProbeStart,
{
    pns_application::decide(
        probes,
        selection,
        overrides,
        pns_domain::DecisionRequest {
            observation: false,
            silence_policy: pns_domain::SilencePolicy::Respect,
            local_only,
            remote_only,
            pane,
            now_secs,
            long_running,
            mobile_watch_card,
        },
    )
}

pub use pns_application::operator_surface;

#[cfg(test)]
mod tests;
