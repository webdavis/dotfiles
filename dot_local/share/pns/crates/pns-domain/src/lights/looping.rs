//! Whether a loop is running, from a lease, a streak or a shell command.

use super::held::marker_is_live;
use super::streak::Streak;

/// Everything the loop condition is a function of.
///
/// A NAMED STRUCT rather than six positional arguments, four of which are
/// `u64`-shaped: a transposition between the two thresholds, or between `now`
/// and either of them, is a lamp judged against the wrong clock and nothing
/// would catch it.
pub struct Loop<'reading> {
    /// The AGENTS' run in progress, which is the only source whose start has to
    /// be inferred: herdr answers a status word and no clock, so the run is
    /// timed from the first tick that read one working.
    pub streak: Option<&'reading Streak>,
    /// Whether any agent is working right now.
    pub agents_working: bool,
    /// When the longest-running tracked shell command STARTED, which is an
    /// exact epoch the shell itself published. It needs no streak: the marker
    /// exists for exactly as long as the command runs.
    pub shell_since: Option<u64>,
    /// When each live lease was last renewed. EMPTY IS THE ORDINARY CASE.
    pub leases: &'reading [u64],
    pub now: u64,
    /// How long tracked work must run continuously before the lamp arms itself.
    pub threshold_secs: u64,
    /// How long a lease survives with nothing renewing it.
    pub lease_timeout_secs: u64,
}
/// Whether the loop lamp is on.
///
/// TWO TRIGGERS AND AN OR, which is the operator's own design: work that has
/// been going long enough arms it by itself, and `pns loop begin` arms it by
/// hand for work whose length nothing can measure in advance. Either is enough,
/// and neither can turn the other off.
///
/// EACH SOURCE IS TIMED AGAINST ITS OWN START, and pooling them was wrong in
/// both directions. The shell publishes the second its command began, so a
/// build is measured from when it really started; an agent gives a status word
/// and nothing else, so its run is timed from the first tick that read it
/// working, and that streak deliberately outlives the work by the grace
/// covering an agent's turn gap. Shared, a fresh five-second command starting
/// inside that grace inherited a finished agent's run and armed the lamp at
/// once, while a build already ten minutes in was clocked from now and had to
/// wait out the whole threshold again.
///
/// BOTH HALVES OF THE AGENT ONE. The streak outliving the work is exactly why
/// the threshold alone would keep the lamp claiming a run in progress after
/// everything went idle.
///
/// AND THE SHELL NEEDS NO SECOND HALF, because its marker exists for exactly as
/// long as its command runs: the reading IS the liveness.
///
/// A `now` BEHIND A START HAS NO ELAPSED TIME IN IT. A clock that stepped
/// backwards would otherwise wrap a subtraction into a huge number that passes
/// every threshold there is.
pub fn loop_running(state: &Loop<'_>) -> bool {
    let long_enough = |since: u64| {
        state
            .now
            .checked_sub(since)
            .is_some_and(|elapsed| elapsed >= state.threshold_secs)
    };
    let agent_run =
        state.agents_working && state.streak.is_some_and(|streak| long_enough(streak.since));
    agent_run
        || state.shell_since.is_some_and(long_enough)
        || state
            .leases
            .iter()
            .any(|at| marker_is_live(*at, state.now, state.lease_timeout_secs))
}
