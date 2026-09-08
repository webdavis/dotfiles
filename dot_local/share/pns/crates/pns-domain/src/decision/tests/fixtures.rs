use crate::registry::Selection;
use crate::surface::{SessionView, Surface};
use crate::{Decision, DecisionRequest, EnvironmentSnapshot, Overrides};

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
    probes: &EnvironmentSnapshot,
    selection: &Selection,
    overrides: &Overrides,
    scope: crate::DeliveryScope,
    pane: &str,
    now_secs: Option<u64>,
    long_running: bool,
    mobile_watch_card: bool,
) -> Decision {
    crate::decide(
        probes,
        selection,
        overrides,
        DecisionRequest {
            silence_policy: crate::SilencePolicy::Respect,
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
pub(super) fn decide_with(
    probes: &EnvironmentSnapshot,
    overrides: &Overrides,
    pane: &str,
) -> Decision {
    decide(
        probes,
        &three_selection(),
        overrides,
        crate::DeliveryScope::Automatic,
        pane,
        Some(1_000_000),
        false,
        false,
    )
}

pub(super) fn operator_surface(
    snapshot: &EnvironmentSnapshot,
    overrides: &Overrides,
    now: Option<u64>,
) -> Surface {
    crate::surface_reading(snapshot, overrides, now).surface
}
