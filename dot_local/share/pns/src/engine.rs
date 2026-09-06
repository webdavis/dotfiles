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
    Wants,
};
use crate::registry::Selection;
use crate::surface::{Surface, Visibility};

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

/// Where the operator is, from the four readings the arbitration needs.
///
/// Public because the blocking hook asks the same question for a different
/// reason: whether the operator can answer from the phone at all.
///
/// EVERY READING IS GUARDED by the verdict that would discard it: a caller who
/// already stated the answer never pays for the probe underneath it.
pub fn operator_surface<P>(probes: &P, overrides: &Overrides, now_secs: Option<u64>) -> Surface
where
    P: IdleProbe + PhoneMarkerProbe + PhoneInputProbe + ScreenLockProbe + ProbeStart,
{
    surface_reading(probes, overrides, now_secs).surface
}

/// The arbitration and the freshness of the reading behind it, in one pass
/// over the probes.
fn surface_reading<P>(probes: &P, overrides: &Overrides, now_secs: Option<u64>) -> SurfaceReading
where
    P: IdleProbe + PhoneMarkerProbe + PhoneInputProbe + ScreenLockProbe + ProbeStart,
{
    // A garbled threshold is UNKNOWN, never the default: substituting 120
    // would read a stale desk as fresh and hold the operator at their desk.
    let desk_fresh_secs = if overrides.desk_invalid {
        None
    } else {
        Some(overrides.desk_idle_secs.unwrap_or(DEFAULT_DESK_IDLE_SECS))
    };
    let Some(desk_fresh_secs) = desk_fresh_secs else {
        // With no window to measure against, nothing can be called fresh,
        // and no reading below this point was ever taken.
        return SurfaceReading {
            surface: Surface::Away,
            phone_input_fresh: false,
            desk_input_age: None,
            phone_input_age: None,
            marker_age: None,
            screen_locked: None,
            desk_fresh_secs: None,
        };
    };

    // ONE START, right where the reads below are about to become certain:
    // the same two predicates the guards below consult, so an override that
    // answers a question outright never starts the probe underneath it.
    probes.start(Wants {
        desk: overrides.reads_desk(),
        phone: overrides.reads_phone(),
    });

    // THE LOCK IS READ ONLY WHERE THE IDLE CLOCK ANSWERED, because its only
    // job is to disqualify what that probe reported: a desk reading the
    // caller stated, never took, or could not take leaves the lock a spawn
    // for an answer nothing can use, and the blocked path an approval waits
    // on pays that deadline serially. Nothing in this repo sets
    // `PNS_IDLE_SECS` in production (measured repo-wide 2026-08-28); a future
    // setter would silently disable the override with it.
    let (desk_input_age, screen_locked) = if overrides.reads_desk() {
        let idle = probes.idle_secs();
        (
            idle,
            idle.is_some().then(|| probes.screen_locked()).flatten(),
        )
    } else if overrides.idle_invalid {
        (None, None)
    } else {
        (overrides.idle_secs, None)
    };
    // AGES, never timestamps, and both aged against the SAME clock read: an
    // unreadable clock ages nothing, which drops a phone signal out of the
    // arbitration rather than making it infinitely fresh.
    let age_of =
        |taken_at: Option<u64>| now_secs.and_then(|now| Some(now.saturating_sub(taken_at?)));
    let phone_input_age = if overrides.reads_phone() {
        age_of(probes.phone_input_atime_secs())
    } else if overrides.phone_invalid {
        None
    } else {
        overrides.phone_input_age
    };
    let marker_age = age_of(probes.marker_mtime_secs());
    SurfaceReading {
        surface: crate::surface::surface(
            desk_input_age,
            phone_input_age,
            marker_age,
            desk_fresh_secs,
            screen_locked,
        ),
        phone_input_fresh: crate::surface::is_fresh(phone_input_age, desk_fresh_secs),
        desk_input_age,
        phone_input_age,
        marker_age,
        screen_locked,
        desk_fresh_secs: Some(desk_fresh_secs),
    }
}

/// Whether the origin pane is on screen. An unreadable view is Unknown, which
/// never suppresses.
fn operator_visibility<P: SessionViewProbe>(probes: &P, pane: &str) -> Visibility {
    if pane.is_empty() {
        return Visibility::Unknown;
    }
    match probes.session_view(pane) {
        Some(view) => crate::surface::visibility(pane, &view),
        None => Visibility::Unknown,
    }
}

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
