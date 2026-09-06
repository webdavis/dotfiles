//! A breath as a cycle of legs, and the fades one tick's budget fits.

/// One leg of a lamp's cycle: the brightness it fades to, and how long that
/// one fade takes.
///
/// THE CYCLE IS A LIST OF THESE, and every shape the lamps run is one. A plain
/// breath is two legs of equal duration; the loop's `breathe_then_flare` is
/// three, the third of them short. Which shape a lamp is running is settled
/// once, where the state is rendered, and everything below this point schedules
/// legs without knowing which shape it was handed.
///
/// EACH LEG CARRIES ITS OWN DURATION rather than the cycle carrying one for all
/// of them. An accent is short by definition, so a single duration per shape
/// would have to be either the accent's or the breath's, and whichever it was
/// the other fade would be issued with the wrong one: told to the bridge as the
/// transition time, and used again to work out when the fade lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Leg {
    pub brightness: u8,
    pub duration_ms: u64,
}
/// One brightness the lamp is asked to fade to, how long that fade takes, and
/// when it is issued.
///
/// `start_ms` IS FROM THE TICK'S OWN START, not from the fade before it, because
/// the driver sleeps against one clock: a per-fade delay accumulates every
/// sleep's own overshoot and the breath drifts past the interval it has.
///
/// `duration_ms` IS THE LEG'S OWN, carried here rather than looked up again by
/// the driver: the fade and the time it takes are one fact, and a driver that
/// reached back to a shape for the second one would state the accent's fade at
/// the breath's duration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fade {
    pub brightness: u8,
    pub duration_ms: u64,
    pub start_ms: u64,
}
/// How far before a fade ends the next one is issued.
///
/// THE SEAMLESS TURN-AROUND, operator-locked on a real lamp: the next fade is
/// issued slightly BEFORE the previous one ends, so the lamp never sits at
/// either end of the breath. Fifty milliseconds is the figure that was set and
/// looked at; nothing here measured what a lead of zero looks like.
pub const FADE_LEAD_MS: u64 = 50;
/// Where a breath resumes: the millisecond its first fade is due, measured
/// from THIS tick's own start, and which leg of the cycle it takes first.
///
/// A ZERO-VALUED `Resume` (due at once, taking leg zero) REPRODUCES THE
/// ORIGINAL, UNBROKEN BREATH: every cycle is built with its LOW leg first, so
/// leg zero is the breath starting down, and a lamp with no record to resume
/// from is a lamp that has never breathed. Starting it down at the tick's
/// first millisecond is the only honest answer for one.
///
/// A NAMED STRUCT, NOT TWO POSITIONAL NUMBERS, because a resume built with the
/// fields swapped would compile and breathe the wrong way from the wrong
/// moment; the two are never interchangeable so the type keeps them apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Resume {
    pub first_due_ms: u64,
    pub next_leg: usize,
}
/// The two legs of a plain breath, LOW FIRST.
///
/// THE ORDER IS THE DEFAULT RESUME'S MEANING: leg zero is what a lamp with no
/// record takes, and the locked answer for one is to start down.
pub fn breath_cycle(breath: &crate::lamps::config::Breath) -> Vec<Leg> {
    vec![
        Leg {
            brightness: breath.low,
            duration_ms: breath.duration_ms,
        },
        Leg {
            brightness: breath.high,
            duration_ms: breath.duration_ms,
        },
    ]
}
/// The three legs of the loop lamp's motion: down, up, and the accent at the
/// peak.
///
/// THE ACCENT FOLLOWS THE RISE AND PRECEDES THE FALL, which is what puts it at
/// the peak: from `low` the lamp rises to `high`, flares briefly past it, and
/// falls from there back to `low`. Operator-locked by eye on a real lamp
/// (2026-09-02) after six other motions were shown and rejected.
pub fn breathe_then_flare_cycle(motion: &crate::lamps::config::BreatheThenFlare) -> Vec<Leg> {
    let mut cycle = breath_cycle(&motion.breath);
    cycle.push(Leg {
        brightness: motion.flare,
        duration_ms: motion.flare_ms,
    });
    cycle
}
/// How long one fade occupies the schedule: its own duration, less the lead
/// the next fade is issued by.
///
/// ONE DEFINITION, read by the schedule that lays the fades out and by the
/// resume that decides how far ahead a recorded phase may honestly sit. Two
/// copies of this arithmetic would let the two disagree about what a step is,
/// and the resume's staleness rule is stated in steps.
pub fn step_ms(duration_ms: u64) -> u64 {
    duration_ms.saturating_sub(FADE_LEAD_MS).max(1)
}
/// The whole breath one tick issues: the fades, in order, with the second one
/// leading the first by `FADE_LEAD_MS` and so on.
///
/// EVERY FADE IS ISSUED STRICTLY INSIDE THE BUDGET, AND THE LAST ONE ENDS
/// AFTER IT. That is what makes the breath seamless rather than paused at its
/// peak: the driver used to stop issuing once a fade's whole DURATION could
/// not fit, which left the lamp holding an end for whatever was left of the
/// interval (a third of it, at the shipped refresh). Ending the schedule at
/// the last ISSUE instead means the lamp is still moving when this tick's
/// child exits; the fade in flight simply keeps running on the bridge with no
/// child left to interrupt it, and the next tick's own first fade is timed to
/// take over `FADE_LEAD_MS` before that one would have ended (`resume_from`
/// reads that from the held record).
///
/// THE RESIDUAL PAUSE, IN FULL: the next tick's own resolve, plus the daemon's
/// second of scheduling slop, plus however far the previous tick's LAST WRITE
/// overran the lead it was issued on, less the slack the schedule already left
/// between that last issue and the budget. Never negative and never more than
/// one step, and zero on most ticks, because the two resolves are of the same
/// order and cancel on average. The write term is the one the schedule cannot
/// see: writes are synchronous, so a bridge that answered the last fade slowly
/// pushes the join out by exactly that much, bounded by the deadline the tick
/// gives each call (a fifth of the interval). So the bound is a ceiling rather
/// than an average, and the lamp holds an end for a fraction of a fade rather
/// than for a third of the interval.
///
/// A RESUME SHIFTS EVERY FADE'S DUE MILLISECOND by `first_due_ms` and picks
/// the leg the cycle carries on from with `next_leg`, so the schedule this
/// tick issues is the next stretch of the breath the previous tick was already
/// running, not a fresh one restarted at the interval's zero.
///
/// THE CYCLE IS WALKED, NOT ALTERNATED. Two legs and three schedule the same
/// way, which is what lets the loop's accent be a leg rather than a special
/// case threaded through the driver: the step each fade occupies is its OWN
/// leg's, so a 200ms accent takes 150ms of the schedule while the 4000ms legs
/// around it take 3950ms each.
///
/// A SCHEDULE THAT WOULD START AT OR PAST THE BUDGET IS EMPTY, which is the
/// same honest answer a schedule with no room for even one fade always gave:
/// the lamp keeps whatever it was last told and the next tick, with its whole
/// interval ahead of it, picks the breath back up.
pub fn breath_fades(budget_ms: u64, cycle: &[Leg], resume: Resume) -> Vec<Fade> {
    if cycle.is_empty() || resume.first_due_ms >= budget_ms {
        return Vec::new();
    }
    let mut fades = Vec::new();
    let mut leg = resume.next_leg % cycle.len();
    let mut start_ms = resume.first_due_ms;
    while start_ms < budget_ms {
        fades.push(Fade {
            brightness: cycle[leg].brightness,
            duration_ms: cycle[leg].duration_ms,
            start_ms,
        });
        start_ms += step_ms(cycle[leg].duration_ms);
        leg = (leg + 1) % cycle.len();
    }
    fades
}
