//! One event's settings, completed observations and pure decision.

use crate::routing::Leg;
use crate::surface::{Surface, Visibility};

mod arbitration;
mod overrides;
mod reading;

pub use arbitration::decide;
pub use overrides::{DEFAULT_DESK_IDLE_SECS, Overrides};
pub use reading::surface_reading;

pub struct DecisionRequest<'a> {
    pub local_only: bool,
    pub remote_only: bool,
    pub pane: &'a str,
    pub now_secs: Option<u64>,
    pub long_running: bool,
    pub mobile_watch_card: bool,
}

#[derive(Default)]
pub struct EnvironmentSnapshot {
    pub idle: Option<u64>,
    pub marker_mtime: Option<u64>,
    pub phone_atime: Option<u64>,
    pub screen_locked: Option<bool>,
    pub view: Option<crate::surface::SessionView>,
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

#[cfg(test)]
mod tests;
