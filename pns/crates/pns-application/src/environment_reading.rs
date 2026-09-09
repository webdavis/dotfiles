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
use pns_domain::registry::Selection;
use pns_domain::surface::Surface;
use pns_domain::{Decision, DecisionRequest, EnvironmentSnapshot, Overrides};

pub fn decide<P>(
    probes: &P,
    selection: &Selection,
    overrides: &Overrides,
    request: DecisionRequest<'_>,
) -> Decision
where
    P: IdleProbe
        + PhoneMarkerProbe
        + PhoneInputProbe
        + ScreenLockProbe
        + SessionViewProbe
        + ProbeStart,
{
    let mut snapshot = read_surface(probes, overrides);
    if !request.pane.is_empty() {
        snapshot.view = probes.session_view(request.pane);
    }
    pns_domain::decide(&snapshot, selection, overrides, request)
}

pub fn operator_surface<P>(probes: &P, overrides: &Overrides, now_secs: Option<u64>) -> Surface
where
    P: IdleProbe + PhoneMarkerProbe + PhoneInputProbe + ScreenLockProbe + ProbeStart,
{
    pns_domain::surface_reading(&read_surface(probes, overrides), overrides, now_secs).surface
}

fn read_surface<P>(probes: &P, overrides: &Overrides) -> EnvironmentSnapshot
where
    P: IdleProbe + PhoneMarkerProbe + PhoneInputProbe + ScreenLockProbe + ProbeStart,
{
    if overrides.desk_invalid {
        return EnvironmentSnapshot::default();
    }
    // ONE START, right where the reads below are about to become certain:
    // the same two predicates the guards below consult, so an override that
    // answers a question outright never starts the probe underneath it.
    probes.start(Wants {
        desk: overrides.reads_desk(),
        phone: overrides.reads_phone(),
    });

    let idle = overrides.reads_desk().then(|| probes.idle_secs()).flatten();
    let screen_locked = idle.is_some().then(|| probes.screen_locked()).flatten();
    let phone_atime = overrides
        .reads_phone()
        .then(|| probes.phone_input_atime_secs())
        .flatten();
    let marker_mtime = probes.marker_mtime_secs();
    EnvironmentSnapshot {
        idle,
        marker_mtime,
        phone_atime,
        screen_locked,
        view: None,
    }
}

#[cfg(test)]
mod tests;
