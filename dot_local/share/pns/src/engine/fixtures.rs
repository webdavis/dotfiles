//! What every decision test builds from: the module's own items, the counting
//! probe set and the recorded readings. One copy, because these rows were one
//! test module before the file outgrew the size rule.

#![allow(unused_imports)]

pub use crate::config::parse_config;
pub use crate::engine::{
    DEFAULT_DESK_IDLE_SECS, Decision, GateInputs, Overrides, SurfaceReading, decide,
    operator_surface,
};
pub use crate::probes::{
    IdleProbe, PhoneInputProbe, PhoneMarkerProbe, ProbeStart, ScreenLockProbe, SessionViewProbe,
    Wants,
};
pub use crate::registry::Selection;
pub use crate::routing::{Leg, ReportMode};
pub use crate::surface::{DeliveryPlan, SessionView, Surface, Visibility};
pub use std::cell::Cell;
pub use std::collections::BTreeMap;

/// Recording probes: every reading is counted, so a test can pin that a
/// probe was never consulted, not only what the verdict was.
#[derive(Default)]
pub(super) struct CountingProbes {
    pub(super) idle: Option<u64>,
    pub(super) marker_mtime: Option<u64>,
    pub(super) phone_atime: Option<u64>,
    pub(super) screen_locked: Option<bool>,
    pub(super) view: Option<SessionView>,
    pub(super) idle_reads: Cell<u32>,
    pub(super) marker_reads: Cell<u32>,
    pub(super) phone_reads: Cell<u32>,
    pub(super) lock_reads: Cell<u32>,
    pub(super) view_reads: Cell<u32>,
    /// What the last `start` call was asked for, synchronous and
    /// nothing to race: this double never spawns a thread, it only
    /// records what it was told.
    pub(super) wants: Cell<Option<Wants>>,
    /// How many times `start` was called: `wants` alone only records
    /// the LAST call, so a caller that starts twice for one event
    /// passes every assertion on `wants` unnoticed.
    pub(super) start_calls: Cell<u32>,
}

impl IdleProbe for CountingProbes {
    fn idle_secs(&self) -> Option<u64> {
        self.idle_reads.set(self.idle_reads.get() + 1);
        self.idle
    }
}
impl PhoneMarkerProbe for CountingProbes {
    fn marker_mtime_secs(&self) -> Option<u64> {
        self.marker_reads.set(self.marker_reads.get() + 1);
        self.marker_mtime
    }
}
impl PhoneInputProbe for CountingProbes {
    fn phone_input_atime_secs(&self) -> Option<u64> {
        self.phone_reads.set(self.phone_reads.get() + 1);
        self.phone_atime
    }
}
impl ScreenLockProbe for CountingProbes {
    fn screen_locked(&self) -> Option<bool> {
        self.lock_reads.set(self.lock_reads.get() + 1);
        self.screen_locked
    }
}
impl SessionViewProbe for CountingProbes {
    fn session_view(&self, _origin_pane: &str) -> Option<SessionView> {
        self.view_reads.set(self.view_reads.get() + 1);
        self.view.clone()
    }
}
impl ProbeStart for CountingProbes {
    fn start(&self, wants: Wants) {
        self.wants.set(Some(wants));
        self.start_calls.set(self.start_calls.get() + 1);
    }
}

/// A view in which the origin pane is on screen, unzoomed.
pub(super) fn watching(origin: &str) -> SessionView {
    SessionView {
        origin_tab: "t1".to_string(),
        focused_tab: "t1".to_string(),
        focused_pane: origin.to_string(),
        zoomed: false,
    }
}

/// A view in which the origin pane's tab is not the one on screen.
pub(super) fn elsewhere(_origin: &str) -> SessionView {
    SessionView {
        origin_tab: "t1".to_string(),
        focused_tab: "t2".to_string(),
        focused_pane: "t2:p9".to_string(),
        zoomed: false,
    }
}

pub(super) fn three_selection() -> Selection {
    crate::registry::roster()
        .enabled(
            &parse_config(
                "[plugins.mobile]\nenabled = true\n[plugins.hermes]\nenabled = true\n[plugins.macos-banner]\nenabled = true\n",
            )
            .unwrap()
            .plugin_switches(),
        )
        .unwrap()
}

pub(super) fn names(decision: &Decision) -> Vec<&str> {
    decision.legs.iter().map(|leg| leg.name).collect()
}

/// One event through the whole engine, with the readings a test cares
/// about and defaults for the rest.
pub(super) fn decide_with(probes: &CountingProbes, overrides: &Overrides, pane: &str) -> Decision {
    decide(
        probes,
        &three_selection(),
        overrides,
        false,
        false,
        pane,
        Some(1_000_000),
        false,
        false,
    )
}
