use crate::*;

/// One lamp's breath for this tick: what to send, and where in its own
/// schedule it resumes.
///
/// A NAMED STRUCT, NOT A TUPLE, once a fourth field (`resume`) joined the
/// three the routing loop already carried: a positional fourth slot is a
/// silent transposition waiting to happen, and every field here already has
/// a name at its own call site.
pub(crate) struct Breathing {
    pub(crate) path: String,
    /// THE STATE THIS BREATH IS SHOWING, carried alongside the shape and the
    /// colour it selected rather than derived back out of them: it is what the
    /// phase is recorded under, and what the next tick compares its own state
    /// against before it resumes anything.
    pub(crate) held: pns::lights::Held,
    /// The legs this lamp fades between, in order. WHICH SHAPE THEY CAME FROM
    /// IS ALREADY SETTLED (`held_render`), so the driver schedules a two-leg
    /// breath and the loop's three-leg motion through one path.
    pub(crate) cycle: Vec<pns::lights::Leg>,
    pub(crate) color: pns::pulse::PulseColor,
    pub(crate) resume: pns::lights::Resume,
}
/// Issue every lamp's breath on cadence for the rest of this interval, and
/// report which end each one landed on and when.
///
/// ONE SLEEP SCHEDULE FOR EVERY LAMP, against one clock. Each fade carries the
/// millisecond it is due at, measured from this function's own start, so a lamp
/// whose write took a moment does not push every later fade of every lamp out by
/// that moment: the overshoot is absorbed rather than accumulated.
///
/// NOTHING IS ISSUED AT OR PAST THE BUDGET, and the check is made immediately
/// before each write rather than once from the schedule. Writes are synchronous
/// and sequential, so the schedule is only ever NOMINAL: four slow lamps due
/// together at 11,850ms with the first taking 150ms puts the rest of that round
/// at or past a 12,000ms budget, and issuing them anyway would hand the bridge
/// fades belonging to an interval this child no longer owns. A dropped fade
/// costs the lamp one turn-around, which the next tick resumes from; an issued
/// one costs two children writing to one lamp.
///
/// AND EVERY LANDING IS DERIVED FROM A WRITE THAT ACTUALLY HAPPENED, at the
/// moment it actually started. The phase this returns is what the next tick
/// resumes off, so a landing taken from the nominal schedule would tell that
/// tick the lamp finished moving earlier than it did, and it would take the
/// breath over early on every interval the bridge ran slow in.
///
/// IT EXITS INSIDE THE BUDGET IT IS HANDED, WITH ITS LAST FADE STILL RUNNING.
/// `breath_fades` issues that fade strictly before the budget ends and lets it
/// finish after, which is the seamless join: the fade keeps moving on the
/// bridge with no child left to interrupt it, and the caller's second held-
/// record write is what lets the next tick pick the join up where this one
/// left it. The budget is what the caller has LEFT of its interval, not the
/// interval, because the map is resolved before the first fade is issued.
///
/// A LAMP WHOSE FADES ARE ALREADY DONE SIMPLY STOPS, which is how lamps with
/// different shapes share one schedule: the blocked lamp's two-second cycles run
/// more often than the unread lamp's four-second one, and the landing each is
/// reported at is exactly the brightness its own last ISSUED fade targeted.
///
/// THE CLOCK AND THE SLEEPER ARE PARAMETERS for one reason: the driver fills its
/// whole interval BY DESIGN, so a test that read the real clock and slept for
/// real would live the interval too. The cadence a fake pair is handed is the
/// same schedule the real one runs.
pub(crate) fn drive_breaths<B: pns::channels::hue::Bridge>(
    bridge: &B,
    budget_ms: u64,
    breathing: &[Breathing],
    mut elapsed_ms: impl FnMut() -> u64,
    mut sleep: impl FnMut(Duration),
) -> Vec<(String, u8, u64)> {
    // (the fade, the lamp it belongs to, its body), in the order they are due.
    let mut schedule: Vec<(pns::lights::Fade, &Breathing, String)> = Vec::new();
    for entry in breathing {
        let fades = pns::lights::breath_fades(budget_ms, &entry.cycle, entry.resume);
        for (index, fade) in fades.iter().enumerate() {
            // THE FIRST FADE CARRIES THE COLOUR AND THE `on`, which is what arms
            // the lamp; every one after it states brightness and duration alone,
            // so the bridge has nothing else to reconcile mid-transition. THIS
            // HOLDS ON A RESUMED TICK TOO: an externally switched-off lamp comes
            // back on with its first fade whichever leg the record names.
            let body = if index == 0 {
                pns::channels::hue::breath_arm_body(entry.color, fade)
            } else {
                pns::channels::hue::fade_body(fade)
            };
            schedule.push((*fade, entry, body));
        }
    }
    schedule.sort_by(|left, right| {
        (left.0.start_ms, &left.1.path).cmp(&(right.0.start_ms, &right.1.path))
    });
    let mut landings: Vec<(String, u8, u64)> = Vec::new();
    for (fade, entry, body) in schedule {
        // SATURATING, so a write that ran long simply issues the next fade at
        // once rather than sleeping a wrapped duration.
        let now_ms = elapsed_ms();
        if fade.start_ms > now_ms {
            sleep(Duration::from_millis(fade.start_ms - now_ms));
        }
        // READ AGAIN AFTER THE SLEEP, because the sleep is the one thing here
        // that is allowed to overshoot, and this is the moment the write starts.
        let at_ms = elapsed_ms();
        if at_ms >= budget_ms {
            break;
        }
        bridge.put(&entry.path, &body);
        // THE FADE'S OWN DURATION, so the accent at the peak of the loop's
        // motion is recorded landing two hundred milliseconds out rather than
        // four seconds out. A landing taken from the shape instead would tell
        // the next tick the lamp finishes moving long after it has, and that
        // tick would hold the lamp still waiting for it.
        let landing = (
            entry.path.clone(),
            fade.brightness,
            at_ms + fade.duration_ms,
        );
        match landings.iter_mut().find(|(path, _, _)| *path == entry.path) {
            Some(previous) => *previous = landing,
            None => landings.push(landing),
        }
    }
    landings
}

#[cfg(test)]
#[path = "lights_breath_runtime/tests.rs"]
mod lights_breath_runtime_tests;
