//! What one event is decided to be: the overrides in force, the readings the
//! arbitration ran on, and the plan that came out.
//!
//! POLICY ONLY: no file, no clock, no environment. `Overrides::from_env` is
//! here because it parses a map somebody else read, not the environment.
//!
//! `decide` itself stays in the legacy package for now. It is generic over the
//! five probe traits, which become ports in a later step; everything it
//! answers IN is here.

use crate::routing::Leg;
use crate::surface::{Surface, Visibility};
use std::collections::BTreeMap;

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
    pub fn reads_desk(&self) -> bool {
        !self.idle_invalid && self.idle_secs.is_none()
    }

    /// The phone twin: whether the phone-input guard would run the
    /// discovery chain instead of trusting a stated or garbled age.
    pub fn reads_phone(&self) -> bool {
        !self.phone_invalid && self.phone_input_age.is_none()
    }

    /// Parse the PNS_* and PNS_* variables out of an environment map.
    pub fn from_env(vars: &BTreeMap<String, String>) -> Self {
        // A present-but-garbled value is reported alongside the None, so the
        // caller can refuse it rather than fall back to a default.
        let read = |key: &str| match vars.get(key).filter(|raw| !raw.is_empty()) {
            None => (None, false),
            Some(raw) => {
                let parsed = crate::count::parse_count(raw);
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
pub struct SurfaceReading {
    pub surface: Surface,
    /// The phone's pty clock is fresh: moshi is open and taking input. False
    /// on a Mobile surface means the Back Tap alone put the operator there.
    pub phone_input_fresh: bool,
    /// THE FOUR RAW READINGS AND THE WINDOW THEY WERE JUDGED AGAINST, carried
    /// out beside the verdict rather than dropped. Nothing downstream may
    /// re-read them: a second reading is a second moment.
    pub desk_input_age: Option<u64>,
    pub phone_input_age: Option<u64>,
    pub marker_age: Option<u64>,
    pub screen_locked: Option<bool>,
    pub desk_fresh_secs: Option<u64>,
}
