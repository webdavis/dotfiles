use crate::ports::environment::{
    IdleProbe, PhoneInputProbe, PhoneMarkerProbe, ProbeStart, ScreenLockProbe, SessionViewProbe,
    Wants,
};
use pns_domain::registry::Selection;
use pns_domain::surface::SessionView;
use pns_domain::{Decision, DecisionRequest, Overrides};
use std::cell::Cell;

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

pub(super) fn three_selection() -> Selection {
    pns_domain::registry::roster()
        .enabled(&std::collections::BTreeMap::from([
            ("mobile".to_string(), true),
            ("hermes".to_string(), true),
            ("macos-banner".to_string(), true),
        ]))
        .unwrap()
}

pub(super) fn names(decision: &Decision) -> Vec<&str> {
    decision.legs.iter().map(|leg| leg.name).collect()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn decide(
    probes: &CountingProbes,
    selection: &Selection,
    overrides: &Overrides,
    scope: pns_domain::DeliveryScope,
    pane: &str,
    now_secs: Option<u64>,
    long_running: bool,
    mobile_watch_card: bool,
) -> Decision {
    crate::environment_reading::decide(
        probes,
        selection,
        overrides,
        DecisionRequest {
            silence_policy: pns_domain::SilencePolicy::Respect,
            scope,
            pane,
            now_secs,
            long_running,
            mobile_watch_card,
        },
    )
}

/// One event through the whole engine, with the readings a test cares
/// about and defaults for the rest.
pub(super) fn decide_with(probes: &CountingProbes, overrides: &Overrides, pane: &str) -> Decision {
    decide(
        probes,
        &three_selection(),
        overrides,
        pns_domain::DeliveryScope::Automatic,
        pane,
        Some(1_000_000),
        false,
        false,
    )
}
