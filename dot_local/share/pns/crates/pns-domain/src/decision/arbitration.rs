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
        scope,
        pane,
        now_secs,
        long_running,
        mobile_watch_card,
        silence_policy,
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
        scope,
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
    // A configured class can preserve the banner and phone already selected
    // above. Silence still suppresses the pulse; caller scope and presence
    // remain authoritative, and the durable log stays outside this plan.
    let delivery = if overrides.silenced() {
        crate::surface::DeliveryPlan {
            banner: silence_policy == super::SilencePolicy::BypassBannerAndPhone && delivery.banner,
            phone_card: silence_policy == super::SilencePolicy::BypassBannerAndPhone
                && delivery.phone_card,
            pulse: false,
        }
    } else {
        delivery
    };
    Decision {
        legs: crate::routing::channel_plan(selection, scope, delivery),
        plan: delivery,
        pane_dropped: !pane.is_empty() && !crate::safety::pane_is_safe(pane),
        inputs: world,
    }
}
