//! The operator's own machine, read into the two verdicts the arbitration
//! compares: WHERE THEIR EYES ARE, and whether the pane an event came from is
//! on screen.
//!
//! ITS OWN MODULE rather than a use case, because three of them ask it: the
//! event path, the approval gate and the doctor. One reading, one place, so
//! the three cannot drift into disagreeing about where the operator is.
//!
//! EVERY READING IS GUARDED by the verdict that would discard it, which is
//! what the port declarations beside this file are shaped for: a caller who
//! already stated the answer never pays for the probe underneath it.
//! Statements: S085, S089 to S091.

use crate::ports::environment::{
    IdleProbe, PhoneInputProbe, PhoneMarkerProbe, ProbeStart, ScreenLockProbe, SessionViewProbe,
    Wants,
};
use pns_domain::decision::{DEFAULT_DESK_IDLE_SECS, Overrides, SurfaceReading};
use pns_domain::surface::{Surface, Visibility};

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
pub fn surface_reading<P>(
    probes: &P,
    overrides: &Overrides,
    now_secs: Option<u64>,
) -> SurfaceReading
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
        surface: pns_domain::surface::surface(
            desk_input_age,
            phone_input_age,
            marker_age,
            desk_fresh_secs,
            screen_locked,
        ),
        phone_input_fresh: pns_domain::surface::is_fresh(phone_input_age, desk_fresh_secs),
        desk_input_age,
        phone_input_age,
        marker_age,
        screen_locked,
        desk_fresh_secs: Some(desk_fresh_secs),
    }
}

/// Whether the origin pane is on screen. An unreadable view is Unknown, which
/// never suppresses.
pub fn operator_visibility<P: SessionViewProbe>(probes: &P, pane: &str) -> Visibility {
    if pane.is_empty() {
        return Visibility::Unknown;
    }
    match probes.session_view(pane) {
        Some(view) => pns_domain::surface::visibility(pane, &view),
        None => Visibility::Unknown,
    }
}
