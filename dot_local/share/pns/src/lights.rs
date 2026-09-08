//! The lamps' three STATES, and the readings each one is derived from.
//!
//! PURE AND TOTAL, like every other decision module: no network, no files, no
//! clock and no environment. The tick reads the machine at its edge and hands
//! the values in, which is what lets a state be swept a second at a time in a
//! unit test.
//!
//! THE TICK RE-DERIVES EVERY STATE FROM SCRATCH and holds nothing in memory
//! between runs, for the reason the daemon states about itself: a divergence
//! between what a process believes and what the disk says is the class this
//! crate keeps paying for.

// The working-file name grammar moved to `pns-domain`, because the safety
// predicates that read the same names are policy too and cannot reach back
// into this package. `sweep_claim` below still writes the sweep's own suffix.
pub use pns_domain::lights::working_owner;

// THE LIGHTING POLICY moved to `pns-domain`, one file per question it answers.
// What stays here reads or writes something: herdr's JSON, the state codecs,
// the paths under the state directory, and the two argv adaptations.
pub use pns_domain::lights::breath::{
    FADE_LEAD_MS, Fade, Leg, Resume, breath_cycle, breath_fades, breathe_then_flare_cycle, step_ms,
};
pub use pns_domain::lights::held::{
    Held, House, active_held, any_blocked, marker_is_live, pulse_fires, shown,
};
pub use pns_domain::lights::looping::{Loop, loop_running};
pub use pns_domain::lights::mute::{
    MAX_MUTED_PLACES, Muted, NO_CLOCK_FOR_THE_MUTE, bare_mute_secs, muted_after, muted_places,
    muted_report,
};
pub use pns_domain::lights::phase::{
    Action, HeldEntry, Phase, Say, blocked_marker_action, resume_from, say,
};
pub use pns_domain::lights::streak::{Streak, WORKING, any_working, next_streak};
pub use pns_domain::lights::unread::{News, Unread, last_interaction, news_after, unread_arming};

pub use pns_adapters::workspace_agent_statuses;

pub use pns_domain::lights::mute::QuietCommand;

#[cfg(test)]
mod fixtures;

#[cfg(test)]
#[path = "lights/tests/streak.rs"]
mod streak_tests;

#[cfg(test)]
#[path = "lights/tests/unread.rs"]
mod unread_tests;

#[cfg(test)]
#[path = "lights/tests/loop.rs"]
mod loop_tests;

#[cfg(test)]
#[path = "lights/tests/phase.rs"]
mod phase_tests;

#[cfg(test)]
#[path = "lights/tests/mute.rs"]
mod mute_tests;

#[cfg(test)]
#[path = "lights/tests/quiet_command.rs"]
mod quiet_command_tests;
