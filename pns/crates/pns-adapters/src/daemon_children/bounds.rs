use crate::config::MAX_REFRESH_SECS;
use pns_domain::lamps::{LIGHTS_JOB, tick_bridge_deadline};
use std::time::Duration;

/// How many ticks a spawned job may run before it is killed, as a FLOOR.
///
/// THIRTY, so the bound moves with the tick and there is ONE knob rather than
/// two. In production that is thirty seconds, which is generous for the event
/// dispatch most of these children are: every channel inside one already
/// carries its own deadline, so a child still alive at this point is wedged
/// rather than slow. The LIGHTS tick is the exception, and `child_bound` is
/// where its own arithmetic lives.
const CHILD_TICKS: u32 = 30;

/// How long a spawned job may actually run before it is killed.
///
/// THE LIGHTS TICK IS THE ONE JOB WHOSE WORK IS AN INTERVAL, and it is named
/// here rather than generalised over every repeat. Every other child is an
/// event delivery whose channels each carry their own deadline, so one still
/// alive at `CHILD_TICKS` is wedged rather than slow and the tick-scaled bound
/// is exactly right for it. Widening the floor to all of them would only make a
/// wedged delivery take longer to kill.
///
/// THE TICK'S OWN ARITHMETIC, STATED: the longest interval it can be given
/// (`MAX_REFRESH_SECS`, thirty seconds), plus the longest a single write may
/// take at that interval (`tick_bridge_deadline`, a fifth of it, so six), plus
/// one reap tick, because a child is only noticed as gone on the pass after it
/// exits. Thirty-seven seconds at the production clock.
///
/// WHY IT IS NOT `CHILD_TICKS` ALONE: that made the tick's child life equal to
/// the longest interval a tick can be given, and a seamless breath issues its
/// last fade strictly INSIDE that interval and lets it finish after. At a
/// thirty-second refresh with 749ms spent resolving, the last write starts at
/// child time 29,999ms and its legal six-second reply was killed before the
/// tick could record where the lamp landed, leaving the next tick to resume
/// from a phase nothing had written. `max` keeps the tick-scaled bound wherever
/// it is the larger of the two, so a deliberately slow clock still gets the
/// generous child it always had.
pub(super) fn child_bound(tick: Duration, id: &str) -> Duration {
    if id != LIGHTS_JOB {
        return tick * CHILD_TICKS;
    }
    let one_lights_tick =
        Duration::from_secs(MAX_REFRESH_SECS) + tick_bridge_deadline(MAX_REFRESH_SECS) + tick;
    (tick * CHILD_TICKS).max(one_lights_tick)
}

#[cfg(test)]
#[path = "bounds/tests.rs"]
mod daemon_child_runtime_tests;
