//! Whether any agent is working, and the streak that says for how long.

/// The one word herdr's agent-status enum uses for a loop that is running.
///
/// The enum is `idle`, `working`, `blocked`, `unknown`, read off the binary's
/// own serde variant table on 0.8.2. Only `working` lights a lamp: `blocked`
/// is the operator's turn, which is the BLOCKED lamp's business, and the other
/// two are nothing happening.
pub const WORKING: &str = "working";
/// Whether ANYTHING is working: any workspace herdr calls `working`, or a
/// plain long command the shell is holding a marker open for.
///
/// AN OR, WHICH IS THE OPERATOR'S OWN AGGREGATION RULE applied literally:
/// the loop lamp breathes if at least one thing is working, and goes dark only
/// when none of them is. An AND would leave the lamp dark for the whole of a
/// single agent's run, which is precisely the run it exists to show.
///
/// THE SHELL MARKER IS THE SECOND PRODUCER and it is a plain epoch: the shell
/// records a long command's start (`dot_bashrc.tmpl` already writes epochs
/// this way) and removes it when the command ends. It is not read for its
/// value here, only for its presence; the streak below is what turns presence
/// into a duration, and it must be the SAME streak the workspaces feed, or a
/// build and an agent loop would each start a clock of their own.
pub fn any_working(agent_statuses: &[String], shell_command_since: Option<u64>) -> bool {
    shell_command_since.is_some() || agent_statuses.iter().any(|status| status == WORKING)
}
/// How long something has been working: when the run started, and when it was
/// last CONFIRMED still going.
///
/// TWO NUMBERS AND NOT ONE, because a streak has to answer two questions that
/// move at different times. `since` is what the breathe threshold measures
/// against and must never move while a loop is alive; `last_seen` is what the
/// grace below measures against and moves on every tick that reads working.
/// One number could carry either meaning and not both.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Streak {
    /// The second the current run of work began.
    pub since: u64,
    /// The second something was last read as working.
    pub last_seen: u64,
}
/// The streak after this tick's reading: started, carried, or cleared.
///
/// THE GRACE IS THE WHOLE POINT OF THE FUNCTION. An agent loop reads
/// not-working in the seconds between its turns, and a streak that reset on
/// the first such reading could never reach a threshold measured in minutes,
/// so the lamp would never breathe at all. The grace is closed at its far
/// edge: exactly `grace_secs` since the last confirmed working second still
/// carries the streak, and one second past it clears.
///
/// A CLEARED STREAK IS GONE RATHER THAN REMEMBERED. The next working reading
/// starts a fresh one at that second, which is what makes "how long has this
/// run been going" answerable at all.
pub fn next_streak(
    held: Option<Streak>,
    working: bool,
    now: u64,
    grace_secs: u64,
) -> Option<Streak> {
    if working {
        return Some(Streak {
            since: held.map_or(now, |streak| streak.since),
            last_seen: now,
        });
    }
    held.filter(|streak| now.saturating_sub(streak.last_seen) <= grace_secs)
}
