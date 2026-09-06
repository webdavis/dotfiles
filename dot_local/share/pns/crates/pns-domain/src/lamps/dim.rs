//! The dim window and which behaviours run their dim form inside it.

use super::window::{QuietWindow, quiet_now};

/// The window a lamp runs dim inside, and which behaviours run dim there.
///
/// THE ENABLES RIDE THE WINDOW, which is what makes them one question: a
/// declaration either states when the lamp is quiet and what it does then, or
/// it says nothing about quiet hours at all. Two separately inherited halves
/// would let a lamp take its room's window and a zone's enables.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DimWindow {
    pub window: QuietWindow,
    /// EMPTY IS SUPPRESS EVERYTHING, and it needs no second mode to spell: a
    /// window with nothing enabled takes every behaviour away for its duration,
    /// which is the bedroom rule with no special case in the code.
    pub behaviours: Vec<crate::lamps::config::Behaviour>,
}
/// What a lamp does with one behaviour right now.
///
/// THREE ANSWERS RATHER THAN A BOOLEAN, because a dim window no longer means one
/// thing: inside it a behaviour either runs its dim form or is taken away
/// entirely, and the caller has to know which body to write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Showing {
    Dark,
    Full,
    Dimmed,
}
/// Which of the three a lamp shows, given the minute of the local day.
///
/// PER BEHAVIOUR, WHICH IS THE WHOLE DESIGN. Inside the window a behaviour the
/// operator enabled runs its dim form and one they did not is suppressed, so a
/// room can breathe faintly about a wait all night while refusing to strobe
/// green about a build. A window with nothing enabled suppresses everything,
/// which is the bedroom rule and needs no mode of its own.
///
/// A LAMP WITH NO WINDOW IS UNTOUCHED. That is what makes the whole feature
/// opt-in: a config that never states a window pays nothing and behaves exactly
/// as it did.
///
/// AN UNREADABLE CLOCK IS INSIDE THE WINDOW, through `quiet_now`'s own rule: a
/// flash at 3am is what the window was set to prevent, and a missed signal
/// costs nothing.
pub fn dim_showing(
    dim: Option<&DimWindow>,
    behaviour: crate::lamps::config::Behaviour,
    minutes_now: Option<u16>,
) -> Showing {
    let Some(dim) = dim else {
        return Showing::Full;
    };
    if !quiet_now(Some(&dim.window), minutes_now) {
        return Showing::Full;
    }
    if dim.behaviours.contains(&behaviour) {
        Showing::Dimmed
    } else {
        Showing::Dark
    }
}
