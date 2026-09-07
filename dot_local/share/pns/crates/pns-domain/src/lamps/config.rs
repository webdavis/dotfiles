//! The plain value types the lamp policy shares with the config edge that
//! parses them. No parsing lives here: a `[lights]` table is read at the edge
//! and arrives as these.

use std::collections::BTreeMap;

/// What a lamp can say. A CLOSED SET, which is the whole reason `[lights]` is
/// judged here instead of passed through as a plugin's free-form settings: a
/// `shows` list holding a word nothing matches is a lamp that stays dark while
/// the operator is sure they routed it, with no message anywhere.
///
/// `Unread` IS ONE WORD AND CARRIES TWO COLOURS. Its success and failure
/// flavours always ride the same lamp, so a config cannot route one without the
/// other and there is no spelling for trying.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Behaviour {
    Done,
    Failed,
    Blocked,
    Unread,
    Looping,
}

/// A breath: how long ONE fade takes, and the two ends it fades between.
///
/// `high` IS THE PEAK. The held record tracks which end a breath last landed
/// on (`resume_from` in `lights.rs`), and every fade the driver issues moves
/// toward one of these two named values, which is why `low` above `high` is
/// refused at load: with the ends reversed, a fade to `high` would move the
/// lamp DOWN and one to `low` would move it up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Breath {
    pub duration_ms: u64,
    pub high: u8,
    pub low: u8,
}

/// The loop lamp's motion: a breath with an accent at its peak.
///
/// THE LOOP'S OWN SHAPE TYPE, and not two more fields on `Breath`, for the
/// config ruling stated at `Lights`: only the knobs that APPLY to a behaviour
/// exist. `Breath` is what the blocked lamp, both unread lamps and the shared
/// dim form run, none of which flare, so an accent parked on `Breath` would be
/// four dead knobs on three behaviours for a reader to set and watch do
/// nothing.
///
/// IT COMPOSES A `Breath` RATHER THAN RESTATING ONE. The two fades either side
/// of the accent are an ordinary breath and are parsed, bounded and checked by
/// the same arm every other breathing shape uses; the accent is the only thing
/// this type adds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BreatheThenFlare {
    pub breath: Breath,
    /// The brightness the accent reaches, above the breath's own `high`.
    pub flare: u8,
    /// How long the accent takes, which is what makes it a flash rather than a
    /// third fade.
    pub flare_ms: u64,
}

/// The lamps' policy: how often a state is re-armed, what each of the five
/// behaviours looks like, and which lamps carry which of them.
///
/// THE TABLE IS OPTIONAL AND ITS ABSENCE IS NOT ITS DEFAULT, which is why the
/// config holds an `Option` of this rather than the struct. A machine with no
/// `[lights]` table keeps the room-based pulse it has always had; a machine
/// with an empty one has asked for the lamps and named no lamp yet. Those are
/// different states and the doctor says different things about them.
///
/// THE DEFAULT IS WRITTEN OUT rather than derived, for `Recap`'s reason: a
/// derived `u64` is zero, and zero is refused by every one of these keys, so a
/// derive would make the empty table unrepresentable through its own parser.
///
/// ONLY THE KNOBS THAT APPLY TO A BEHAVIOUR EXIST (operator ruling): a pulse
/// has a duration and one brightness, a breathing state has a duration and two
/// ends, and some of them carry one knob more besides (unread's delay, loop's
/// threshold and lease, blocked's give-up backstop). There is no dead knob
/// anywhere for a reader to set and watch do nothing.
#[derive(Debug, Clone, PartialEq)]
pub struct Lights {
    pub refresh_secs: u64,
    pub done: Pulse,
    pub failed: Pulse,
    pub blocked: Blocked,
    pub unread: Unread,
    /// `[lights.loop]`. NOT SPELLED `r#loop` AT THE FIELD, because every reader
    /// would then carry the raw identifier through; the TOML key is `loop` and
    /// the mapping is stated once, in `parse_lights`.
    pub looping: Looping,
    /// The one dim FORM, shared by every behaviour that runs dimmed, because
    /// the operator locked one shape rather than one per behaviour. WHICH
    /// behaviours run it is a per-target opt-in, not a knob here.
    pub dim: Breath,
    pub lamps: BTreeMap<String, Target>,
    pub rooms: BTreeMap<String, Target>,
    pub zones: BTreeMap<String, Target>,
}
/// A blink: how long the bridge runs it, and how bright.
///
/// NO LOW, because a pulse has no low to run to. That is the config ruling
/// applied at the type level rather than in a comment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pulse {
    pub duration_ms: u64,
    pub brightness: u8,
}
/// The blocked lamp: its breath, plus how long an unanswered wait may hold it
/// before the daemon gives up on an abandoned session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Blocked {
    pub breath: Breath,
    pub give_up_after_secs: u64,
}
/// The unread lamp: its breath, plus how old SUCCESS news must be before it
/// arms. Failure news arms with no delay at all and has no knob.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unread {
    pub breath: Breath,
    pub after_secs: u64,
}
/// The loop lamp: its motion, how long work must run before the automatic
/// trigger arms, and how long a hand-taken lease survives without renewal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Looping {
    pub breathe_then_flare: BreatheThenFlare,
    pub threshold_secs: u64,
    pub lease_timeout_secs: u64,
}
/// One declaration, at one of the three levels, and the questions it answers.
///
/// EACH FIELD IS ONE QUESTION, resolved independently of the others: a lamp's
/// own declaration can state which behaviours it carries and say nothing about
/// dimming, and its room's window still applies. `Option` is what spells "said
/// nothing" for the behaviour set; the dim question is stated exactly when
/// `dim_window` is.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Target {
    pub shows: Option<Vec<Behaviour>>,
    pub dim_window: Option<String>,
    /// The behaviours that run their DIM FORM inside that window. Everything
    /// else the target carries is suppressed there, which is what makes a
    /// window with an empty list a room that goes dark for the night with no
    /// second mode to spell it.
    pub dim_behaviours: Vec<Behaviour>,
}
impl Default for Lights {
    fn default() -> Self {
        Lights {
            refresh_secs: DEFAULT_REFRESH_SECS,
            done: DEFAULT_DONE,
            failed: DEFAULT_FAILED,
            blocked: Blocked {
                breath: DEFAULT_BLOCKED,
                give_up_after_secs: DEFAULT_BLOCKED_GIVE_UP_AFTER_SECS,
            },
            unread: Unread {
                breath: DEFAULT_UNREAD_BREATH,
                after_secs: DEFAULT_UNREAD_AFTER_SECS,
            },
            looping: Looping {
                breathe_then_flare: DEFAULT_LOOP_MOTION,
                threshold_secs: DEFAULT_LOOP_THRESHOLD_SECS,
                lease_timeout_secs: DEFAULT_LEASE_TIMEOUT_SECS,
            },
            dim: DEFAULT_DIM,
            lamps: BTreeMap::new(),
            rooms: BTreeMap::new(),
            zones: BTreeMap::new(),
        }
    }
}
/// How often a lamp holding a state is re-armed.
///
/// TWELVE, and it is a breath budget rather than a round number: the tick's own
/// driver fades a breathing lamp seamlessly across the whole interval, so the
/// interval is what decides how many fades fit between two ticks. The count is
/// the budget divided by a fade's own step (its duration less the seamless
/// lead), rounded UP, because the last fade is issued inside the interval and
/// lands after it: twelve seconds carries seven of the locked two-second
/// shape, and three or four of the four-second one depending on what that
/// tick's resolve took off the budget first.
pub const DEFAULT_REFRESH_SECS: u64 = 12;
/// The five locked shapes. EVERY NUMBER HERE WAS SET ON A REAL LAMP under the
/// operator's observe-adjust-lock protocol (2026-08-31 and 2026-09-01), so a
/// change to one of them is a change to something that was looked at, not a
/// tuning.
pub const DEFAULT_DONE: Pulse = Pulse {
    duration_ms: 4000,
    brightness: 100,
};
pub const DEFAULT_FAILED: Pulse = Pulse {
    duration_ms: 4000,
    brightness: 100,
};
pub const DEFAULT_BLOCKED: Breath = Breath {
    duration_ms: 2000,
    high: 100,
    low: 30,
};
pub const DEFAULT_UNREAD_BREATH: Breath = Breath {
    duration_ms: 4000,
    high: 60,
    low: 10,
};
/// The loop's motion, operator-locked by eye on a real Studio lamp
/// (2026-09-02) after six other motions were shown and rejected: from 10, rise
/// to 80 over four seconds, flash to 100 for two hundred milliseconds at the
/// peak, and fall back to 10 over four seconds.
pub const DEFAULT_LOOP_MOTION: BreatheThenFlare = BreatheThenFlare {
    breath: Breath {
        duration_ms: 4000,
        high: 80,
        low: 10,
    },
    flare: 100,
    flare_ms: 200,
};
/// The dim form: the same seamless cadence at the faintest levels the hardware
/// has. Drill D4 measured a lamp asked for one percent reporting 1.19, which is
/// its own floor rather than a rounding.
pub const DEFAULT_DIM: Breath = Breath {
    duration_ms: 3000,
    high: 7,
    low: 1,
};
/// How old SUCCESS news must be before the unread lamp arms: five minutes, so a
/// result the operator is already looking at does not light a lamp about itself.
/// FAILURE news has no such delay and no knob.
pub const DEFAULT_UNREAD_AFTER_SECS: u64 = 300;
/// How long an unanswered wait may hold the blocked lamp before the daemon
/// gives up on an abandoned session (operator ruling 2026-09-01).
///
/// SIXTEEN HOURS, AND IT IS STILL A BACKSTOP RATHER THAN AN EXPIRY. The locked
/// behaviour is the blocked lamp breathing CONTINUOUS UNTIL THE OPERATOR
/// ANSWERS, so any bound at all is a departure from it and the only honest job
/// left for one is releasing a bulb from a session that will never come back.
/// Sixteen hours outlasts a long day away and still gives the bulb back before
/// the next one starts. The ORDINARY end is not this at all: the session's
/// next event clears the marker, whatever the hour.
pub const DEFAULT_BLOCKED_GIVE_UP_AFTER_SECS: u64 = 16 * 60 * 60;
/// How long work must run continuously before the loop lamp arms itself.
pub const DEFAULT_LOOP_THRESHOLD_SECS: u64 = 300;
/// How long a hand-taken loop lease survives with nothing renewing it.
///
/// SIXTY-FIVE MINUTES, and the number comes from what renews it: the lease is
/// refreshed by the calling pane's ordinary hook traffic, and the harness's own
/// wakeup scheduler clamps a sleep to 3600 seconds, so the longest legitimate
/// gap between two events from a live loop is an hour. A timeout at the hour
/// itself would drop a lease that was about to be renewed.
pub const DEFAULT_LEASE_TIMEOUT_SECS: u64 = 3900;
