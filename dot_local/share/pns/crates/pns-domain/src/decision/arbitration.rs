use super::reading::operator_visibility;
use super::{
    Decision, DecisionRequest, EnvironmentSnapshot, GateInputs, Overrides, surface_reading,
};
use crate::registry::Selection;

pub fn decide(
    snapshot: &EnvironmentSnapshot,
    selection: &Selection,
    overrides: &Overrides,
    request: DecisionRequest<'_>,
) -> Decision {
    let DecisionRequest {
        local_only,
        remote_only,
        pane,
        now_secs,
        long_running,
        mobile_watch_card,
    } = request;
    let reading = surface_reading(snapshot, overrides, now_secs);
    let session_visibility = operator_visibility(snapshot, pane);
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
