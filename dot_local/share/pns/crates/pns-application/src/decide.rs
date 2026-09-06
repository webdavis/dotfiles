//! The delivery decision for one event: which legs it reaches, and why.
//!
//! ASSEMBLY ONLY. Where the operator is looking is the domain's `surface`,
//! whether the origin pane is on screen is its `visibility`, and what to do
//! about it is its `plan`. This reads the probes those three need and turns
//! the plan into legs.
//!
//! HERE RATHER THAN IN THE DOMAIN, because it is generic over the probe ports
//! beside it. The domain answers from readings it is handed; this is what
//! takes them.

use crate::ports::environment::{
    IdleProbe, PhoneInputProbe, PhoneMarkerProbe, ProbeStart, ScreenLockProbe, SessionViewProbe,
};
use pns_domain::decision::{Decision, GateInputs, Overrides};
use pns_domain::registry::Selection;

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
    let reading = crate::environment_reading::surface_reading(probes, overrides, now_secs);
    let session_visibility = crate::environment_reading::operator_visibility(probes, pane);
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
        visibility: pns_domain::surface::effective_visibility(
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
    let delivery = pns_domain::surface::plan(
        world.surface,
        world.visibility,
        long_running,
        mobile_watch_card,
    );
    // The two caller overrides survive the arbitration they used to steer:
    // skip beats force, and both beat the surface.
    let delivery = pns_domain::surface::DeliveryPlan {
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
        pns_domain::surface::DeliveryPlan {
            banner: false,
            phone_card: false,
            pulse: false,
        }
    } else {
        delivery
    };
    Decision {
        legs: pns_domain::routing::channel_plan(selection, local_only, remote_only, delivery),
        plan: delivery,
        pane_dropped: !pane.is_empty() && !pns_domain::safety::pane_is_safe(pane),
        inputs: world,
    }
}
