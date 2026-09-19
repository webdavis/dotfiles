//! The stale-block escalation: what a session blocked too long IS, when the
//! operator could act on one, and what the page about it says.
//!
//! POLICY ONLY. Every function here is a total function of its arguments, with
//! no clock, no config, no filesystem and no printing. The query that finds
//! these rows is the persistence adapter's, the job that wakes the fire is the
//! daemon's, and the surface reading the gate judges is taken by the caller.
//!
//! IT IS THE REMINDER'S SIBLING AND NOT A SETTING ON IT (design, 2026-09-14).
//! The reminder says "this approval is still waiting" minutes later on the
//! ordinary route; this says "nobody is coming" an hour later on the route
//! reserved for things that need a human, once per block.
//!
//! WHICH ROUTE THAT IS stays the operator's to name. `[stale] route` names
//! one outright and reaches `page` as an argument; with none named the page
//! carries the HEALTH kind alone, so the one statement that turns a kind into
//! a route (`routes::Kind::route`) settles it. This module names no route of
//! its own either way.

use crate::surface::Surface;

/// One session whose wait is older than the window, as the row names it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Blocked {
    pub session: String,
    pub harness: String,
    pub project: String,
    pub branch: String,
    pub title: String,
    /// The second the wait started, which is the other end of the measurement.
    pub since: u64,
}

/// The daemon job's id for one session. ONE JOB PER BLOCK, and the id is the
/// spool filename, so a new block REPLACES the job rather than stacking a
/// second one.
///
/// A COLON, which `session_id_is_safe` refuses and the daemon's own id rule
/// admits, so a job id can never be mistaken for a session id. It is
/// `remind::usable`'s bound that decides whether a session can carry a name at
/// all, because both prefixes are measured against the same daemon cap.
pub fn job_id(session_id: &str) -> Option<String> {
    crate::remind::usable(session_id).map(|id| format!("{JOB_PREFIX}{id}"))
}

/// What an escalation job's id starts with.
const JOB_PREFIX: &str = "stale:";

/// The word the daemon re-executes this binary with.
pub const FIRE_WORD: &str = "stale";

/// The state word the page carries, which is also the word that started the
/// wait being escalated.
const BLOCKED_STATE: &str = "blocked";

/// Whether the operator could act on a page right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    Page,
    Skip(Suppressed),
}

/// Why a fire said nothing. TWO REASONS, NOT ONE, because they send a reader
/// to two different places: away is an operator who is not at a keyboard, and
/// a screen locked through the window is a machine that has been asleep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Suppressed {
    Away,
    LockedThroughWindow,
}

impl Suppressed {
    /// The half-sentence the fire's own line carries.
    pub fn said(self) -> &'static str {
        match self {
            Suppressed::Away => "the operator is away",
            Suppressed::LockedThroughWindow => "the screen was locked for the whole window",
        }
    }
}

/// Whether this moment earns a page, from ONE surface reading.
///
/// AWAY IS A SKIP because the thing a blocked agent needs is a keyboard, and
/// `Away` means neither the desk nor the phone has a fresh reading, so there
/// is nobody at one. A page nobody can act on is a notification that trains
/// the operator to ignore the route reserved for the ones they must.
///
/// A SCREEN LOCKED THROUGH THE WINDOW IS A SKIP, and ONE INSTANTANEOUS READING
/// ANSWERS A QUESTION ABOUT AN HOUR: unlocking a Mac takes input and input
/// resets the idle clock, so a desk idle for the whole window cannot have been
/// unlocked inside it. pns keeps no history of the lock, which is why the idle
/// age is what carries the duration.
///
/// A SCREEN LOCKED FOR PART OF THE WINDOW STILL PAGES, which is the intended
/// direction: the operator was there, stepped away, and the page is what tells
/// them a session is stuck.
///
/// ONLY `Some(true)` LOCKS, matching `surface`'s own rule, so an `ioreg` that
/// stops answering costs the suppression rather than the page. An unknown idle
/// age is the same direction: a lock nothing can measure a duration against
/// cannot prove the window, so it does not suppress.
pub fn gate(
    surface: Surface,
    screen_locked: Option<bool>,
    desk_input_age: Option<u64>,
    window: u64,
) -> Gate {
    if surface == Surface::Away {
        return Gate::Skip(Suppressed::Away);
    }
    if screen_locked == Some(true) && desk_input_age.is_some_and(|age| age >= window) {
        return Gate::Skip(Suppressed::LockedThroughWindow);
    }
    Gate::Page
}

/// The page: the ordinary event of the sender header, on the route the fire
/// was given, whose detail is how long the block has stood.
///
/// THE HEADER AND SUBHEADER ARE COMPOSED WHERE EVERY OTHER EVENT'S ARE, off
/// these fields at the hermes destination, so a page reads as the same line
/// the `pns` channel already carries for that session rather than a second
/// format nobody recognises.
///
/// NO PANE. The row carries none, because the fire is a daemon child with no
/// `HERDR_PANE_ID` of its own and the sessions row is not where a pane
/// belongs; a page that focuses nothing is honest, and an invented pane id
/// would focus somebody else's.
pub fn page(blocked: &Blocked, now: u64, route: &str) -> crate::EventArgs {
    crate::EventArgs {
        agent: blocked.harness.clone(),
        state: BLOCKED_STATE.to_string(),
        project: blocked.project.clone(),
        branch: blocked.branch.clone(),
        detail: waited(now.saturating_sub(blocked.since)),
        // THE ROUTE THE OPERATOR NAMED, which `[stale] route` settles and the
        // caller has already resolved; empty leaves the kind below to pick it.
        channel: route.to_string(),
        kind: crate::routes::Kind::Health,
        session: blocked.session.clone(),
        session_title: blocked.title.clone(),
        ..crate::EventArgs::default()
    }
}

/// How long the block has stood, as the page says it.
///
/// SPELLED OUT IN MINUTES rather than `remind::waited`'s compact `63m` (design,
/// 2026-09-14). That one is read by someone who has been watching the pane it
/// is about; this one is read by someone who has not, on the route they have
/// agreed to be interrupted on, where a bare unit suffix is one more thing to
/// decode.
///
/// SATURATING, and a wait the clock cannot make sense of reads as zero: the
/// stamps come off disk, so this arithmetic is not entitled to assume they are
/// ordered.
pub fn waited(seconds: u64) -> String {
    let minutes = seconds / 60;
    if minutes == 1 {
        return "blocked 1 minute, no answer".to_string();
    }
    format!("blocked {minutes} minutes, no answer")
}

#[cfg(test)]
mod tests;
