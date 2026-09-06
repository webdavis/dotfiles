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

// THE DECISION'S VALUE TYPES moved to `pns-domain`. `decide` stays here until
// the probe traits it is generic over become ports.
pub use pns_domain::decision::{
    DEFAULT_DESK_IDLE_SECS, Decision, GateInputs, Overrides, SurfaceReading,
};

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
    let reading = surface_reading(probes, overrides, now_secs);
    let session_visibility = operator_visibility(probes, pane);
    // EVERY FIELD IS STATED HERE, once. The event-shaped half cannot be
    // filled by the reading above, and a struct assembled in two places is
    // one a later edit can leave holding a default nobody meant.
    let world = GateInputs {
        desk_input_age: reading.desk_input_age,
        phone_input_age: reading.phone_input_age,
        marker_age: reading.marker_age,
        screen_locked: reading.screen_locked,
        desk_fresh_secs: reading.desk_fresh_secs,
        surface: reading.surface,
        session_visibility,
        // The session reports one fact for every client, and a phone with
        // moshi closed is not one of them: see `surface::effective_visibility`.
        visibility: crate::surface::effective_visibility(
            reading.surface,
            reading.phone_input_fresh,
            session_visibility,
        ),
        now_secs,
        long_running,
        mobile_watch_card,
        local_only,
        remote_only,
        pane_present: !pane.is_empty(),
    };
    let delivery = crate::surface::plan(
        world.surface,
        world.visibility,
        long_running,
        mobile_watch_card,
    );
    // The two caller overrides survive the arbitration they used to steer:
    // skip beats force, and both beat the surface.
    let delivery = crate::surface::DeliveryPlan {
        phone_card: !overrides.skip_phone && (overrides.force_phone || delivery.phone_card),
        ..delivery
    };
    // THE TWO MUTES, applied LAST and therefore beating `PNS_FORCE_PHONE`
    // above them. Force is a producer's per-event opinion set in the
    // environment; the operator's mute is their own typed, expiring
    // instruction, and a macOS Focus they named in `[focus] silence` is the
    // same instruction with the operating system as its author. A mute any
    // producer can override is not a mute.
    //
    // ONE CONDITION FOR BOTH, so every downstream property (the journal, the
    // deferred replay, beating force, the decision log) follows from one rule
    // rather than from two that could drift. The durable log is not a field of
    // `DeliveryPlan`, so the record survives both of them structurally.
    //
    // A FULL STRUCT LITERAL WITH NO `..delivery`, deliberately: it is what
    // forces a future field of `DeliveryPlan` to state its own answer here
    // rather than inherit an unmuted one. Do not tidy it into a struct update.
    let delivery = if overrides.silenced() {
        crate::surface::DeliveryPlan {
            banner: false,
            phone_card: false,
            pulse: false,
        }
    } else {
        delivery
    };
    Decision {
        legs: crate::routing::channel_plan(selection, local_only, remote_only, delivery),
        plan: delivery,
        pane_dropped: !pane.is_empty() && !crate::safety::pane_is_safe(pane),
        inputs: world,
    }
}

/// THE ENVIRONMENT READING moved to `pns-application`, where the probe ports
/// it is generic over are declared. `decide` stays here until it becomes a use
/// case of its own.
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
