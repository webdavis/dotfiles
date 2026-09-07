//! Where a breath resumes, what a marker's state does, and what to say once.

use super::breath::{FADE_LEAD_MS, Leg, Resume, step_ms};
use super::held::Held;

/// Where one lamp's breath left off: the instant its last issued fade lands,
/// the BRIGHTNESS it lands on, and the STATE that breath was showing.
///
/// THE STATE IS PART OF THE PHASE, not a separate question. A phase is only
/// worth resuming while the lamp is still breathing the same shape in the same
/// colour; carried across a state change it delays the new colour by up to one
/// whole fade, because the first fade of every tick is the one that carries
/// the colour and `on`. That is the locked precedence (red wins, blocked
/// outranks loop) arriving late, which is the one thing the resume must never
/// cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Phase {
    pub end_unix_ms: u64,
    /// The brightness the last issued fade was moving toward, which is the leg
    /// of the cycle it was running.
    ///
    /// THE BRIGHTNESS AND NOT THE LEG'S INDEX, because an index means nothing
    /// without the cycle it counts into and the cycle a lamp runs can change
    /// between two ticks: a loop lamp entering its dim window swaps a
    /// three-leg motion for a two-leg one, and index two would then be read as
    /// a leg that no longer exists. A brightness that is not in the new cycle
    /// simply reads as a lamp with nowhere to resume, which starts it fresh.
    ///
    /// TWO LEGS MAY SHARE ONE LEVEL, and the first of them wins. `ends_agree`
    /// refuses only a `low` ABOVE its `high`, so a config may set the two
    /// equal; the accent stays distinct either way, since `accent_agrees`
    /// keeps it above `high`. In that config a record naming the shared level
    /// resumes onto the earlier leg, which costs the cycle one extra leg
    /// before it reaches the accent. That leg fades to the level the lamp is
    /// already at, so there is nothing to see for it.
    pub landed_on: u8,
    pub held: Held,
}
/// One lamp's line in the held record: the fixture path, and where in its
/// breath it left off.
///
/// `resume` IS `None` FOR A BARE PATH, which is a lamp the record holds with
/// no phase attached: a fresh arm, a phase write a race stood down, or a
/// token an older build or a hand edit left without one. All three read the
/// same way, as a breath that starts fresh.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldEntry {
    pub path: String,
    pub resume: Option<Phase>,
}
impl HeldEntry {
    /// A lamp held with no phase recorded for it.
    pub fn bare(path: impl Into<String>) -> HeldEntry {
        HeldEntry {
            path: path.into(),
            resume: None,
        }
    }
}
/// The `Resume` a lamp's next breath starts from, off what its held entry
/// last recorded.
///
/// `first_due_ms` IS THE RECORDED END, LESS THE SEAMLESS LEAD, LESS NOW,
/// SATURATING AT ZERO: the previous tick's last fade does not finish landing
/// on the bridge until that instant, and the next one has to be issued
/// `FADE_LEAD_MS` before it, exactly as every fade inside one tick already is.
/// A `now_ms` past that moment (a tick that ran late, or the bridge holding
/// the lamp at its recorded end since nothing else has moved it) saturates to
/// zero: due at once, not due in the past.
///
/// NO ENTRY AND NO PHASE BOTH GIVE THE DEFAULT `Resume`, which starts the
/// breath down at once: the lamp has never breathed, or something else put it
/// somewhere this record does not describe (an external switch, a killed
/// child's bare token, a dim-window shape change), and starting fresh from the
/// low end costs at most one fade of motion, never a pause.
///
/// AND A PHASE ANOTHER STATE LEFT IS NOT RESUMED FROM, which is the case that
/// costs a PAUSE rather than a fade. The slow shapes land their last fade
/// almost four seconds past the interval that issued them, so a lamp that was
/// looping and is now blocked would wait that fade out before its first blocked
/// body reached the bridge: the locked precedence, arriving up to a whole fade
/// late. A state change starts down at once instead.
///
/// AND A BRIGHTNESS THIS CYCLE DOES NOT RUN IS NOT RESUMED FROM EITHER, which
/// is how a shape change lands here: the record says the lamp was moving toward
/// a level the cycle it is about to run has no leg for, so there is no leg to
/// carry on from and the honest answer is to start the new shape at its own
/// beginning.
///
/// AND A PHASE MORE THAN ONE STEP AHEAD IS STALE, NOT PATIENT. `now_ms` is
/// wall time and so is the recorded end, so an hour lost to a time-zone edit,
/// an NTP correction or a resumed sleep leaves a valid record looking like a
/// fade due an hour from now: a schedule that starts past the budget, issues
/// nothing, and holds the lamp still for a whole interval. The step of the leg
/// the record names is the ceiling because it is a law and not a tolerance:
/// the tick that wrote the phase issued that leg's fade strictly inside its
/// own budget, the fade lands one leg-duration later, and the next tick begins
/// at most the daemon's slop after that budget ended, so `first_due_ms` is
/// always under that one leg's step. THE LEG'S OWN AND NOT THE CYCLE'S LONGEST:
/// a 200ms accent cannot honestly leave a fade due 3950ms out, and reading the
/// law off the leg that was actually running is what keeps the ceiling as tight
/// as the fact it comes from.
pub fn resume_from(entry: Option<&HeldEntry>, now_ms: u64, showing: Held, cycle: &[Leg]) -> Resume {
    let Some(phase) = entry.and_then(|entry| entry.resume) else {
        return Resume::default();
    };
    if phase.held != showing {
        return Resume::default();
    }
    let Some(landed) = cycle
        .iter()
        .position(|leg| leg.brightness == phase.landed_on)
    else {
        return Resume::default();
    };
    let first_due_ms = phase
        .end_unix_ms
        .saturating_sub(now_ms)
        .saturating_sub(FADE_LEAD_MS);
    if first_due_ms > step_ms(cycle[landed].duration_ms) {
        return Resume::default();
    }
    Resume {
        first_due_ms,
        next_leg: (landed + 1) % cycle.len(),
    }
}
/// What one harness event does to its session's needs marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// An agent is waiting on the operator from here until something says
    /// otherwise.
    Start,
    /// A later event from that session, which is what says otherwise.
    End,
}
/// Which of the two an event's STATE is.
///
/// A CLOSED SET OF STARTERS AND EVERYTHING ELSE ENDS, rather than a closed set
/// on both sides. A state this does not recognise is still a later event from
/// that session, and the fail direction that matters is the one that lets a
/// lamp go dark: an unknown word treated as a start would hold blocked on a
/// session nobody is waiting for.
///
/// IT READS `pulse::LAMP_BLOCKED`, the list the lamps already carry, and NOT
/// `missed_notifications::NEEDS_YOU`, which correctly includes `failed`. A dead
/// turn is `failed`, not `blocked`, and it is not a wait anybody can end.
pub fn blocked_marker_action(event_state: &str) -> Action {
    if crate::pulse::LAMP_BLOCKED.contains(&event_state) {
        Action::Start
    } else {
        Action::End
    }
}
/// What a tick does with the complaints it has this second.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Say {
    /// Print nothing and change nothing: either there is nothing wrong, or the
    /// same thing is still wrong and it has already been said.
    Nothing,
    /// Print the complaints and remember this line as what was said.
    Aloud(String),
    /// The complaint cleared. Print nothing, and forget, so that the same
    /// complaint coming back is news again.
    Forget,
}
/// Whether this tick's complaints are worth saying, given what the last one
/// said.
///
/// ONCE, NOT EVERY TICK, and the memory is on disk because there is no
/// process to hold it in: the daemon re-executes this binary for every tick,
/// so "once per daemon lifetime" cannot be a variable. This is
/// `remember_staleness`'s idiom one directory over, and its reason is the
/// same: the thing worth saying is a CHANGE.
///
/// ONE LINE, JOINED, because the memory is one state file and every state file
/// in this crate is published as a single line. A complaint carrying a newline
/// is flattened into it, so the memory can never be read back as two.
pub fn say(lines: &[String], remembered: &str) -> Say {
    let said = lines.join(" | ").replace('\n', " ");
    if said == remembered {
        return Say::Nothing;
    }
    if said.is_empty() {
        return Say::Forget;
    }
    Say::Aloud(said)
}
