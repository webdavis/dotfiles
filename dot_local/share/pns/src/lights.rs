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

/// What the operator typed at `pns loop`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoopCommand {
    /// Take a lease for this pane, and hold the lamp until it is given back or
    /// times out.
    Begin(String),
    /// Give it back.
    End(String),
}

/// The typed command, or the refusal that says what was missing.
///
/// THE PANE IS THE OPERATOR'S OWN, TAKEN FROM THE ENVIRONMENT they typed in,
/// because that is what a lease is keyed to: `HERDR_PANE_ID` is set for every
/// pane herdr owns, so the ordinary case needs no argument at all.
///
/// TYPED OUTSIDE A PANE IT IS A REFUSAL, NEVER A GUESS. There is no sensible
/// pane to pick, and picking one would key the lease to a pane whose ordinary
/// traffic will never renew it: the lamp would then breathe for the whole
/// timeout with nothing behind it, which is the exact opposite of a liveness
/// signal.
///
/// AN UNKNOWN ARGUMENT IS AN ERROR, never a silent fallthrough, because a
/// mistyped flag would otherwise be a lease the operator believes they took.
pub fn loop_command(
    verb: &str,
    arguments: &[String],
    env_pane: Option<&str>,
) -> Result<LoopCommand, String> {
    let pane = match arguments {
        [] => env_pane
            .filter(|pane| !pane.is_empty())
            .map(str::to_string)
            .ok_or_else(|| NO_PANE.to_string())?,
        [flag, pane] if flag == "--pane" => pane.clone(),
        _ => return Err(LOOP_USAGE.to_string()),
    };
    if !crate::safety::pane_file_is_safe(&pane) {
        return Err(format!(
            "pns: loop: {pane:?} is not a pane id this can key a lease to"
        ));
    }
    match verb {
        "begin" => Ok(LoopCommand::Begin(pane)),
        "end" => Ok(LoopCommand::End(pane)),
        _ => Err(LOOP_USAGE.to_string()),
    }
}

/// Why a lease cannot be taken with no pane to key it to.
const NO_PANE: &str = "pns: loop: no HERDR_PANE_ID in this environment, so there \
is no pane to key the lease to; run it inside the pane, or name one with --pane";

pub const LOOP_USAGE: &str = "pns: usage: pns loop begin [--pane <id>] | \
pns loop end [--pane <id>]";

pub use pns_domain::lights::mute::QuietCommand;

/// The typed command, or the refusal that quotes back what was typed.
///
/// A PLACE NO CLAIM NAMES IS REFUSED RATHER THAN STORED. A mute is a line in a
/// file that nothing will ever match, so the lamp the operator meant to quiet
/// goes on flashing while the command reports success; the only evidence they
/// get is the lamp itself, at the hour they were trying not to be disturbed.
/// The vocabulary is the caller's `known`, which is every name a mute can
/// ENFORCE at any of the three levels.
///
/// `off` IS ALLOWED OVER ANY NAME, because it can only remove. A place muted
/// yesterday and dropped from the config today would otherwise be a mute
/// nothing could clear, which is the state the refusal exists to prevent rather
/// than to create.
///
/// THE DURATION IS `quiet::parse_duration`'S, refusal and all, so a second
/// spelling of "how long" cannot exist and neither can a second set of bounds.
pub fn quiet_command(
    arguments: &[String],
    known: &[String],
    until_quiet_ends: Option<u64>,
) -> Result<QuietCommand, String> {
    match arguments {
        [] => Ok(QuietCommand::Report),
        [place, word] if word == "off" => Ok(QuietCommand::Unmute {
            place: place.clone(),
        }),
        [place] => {
            if !known.iter().any(|name| name == place) {
                return Err(unmutable(place, known));
            }
            // NO SCHEDULE IS A REFUSAL, never a guessed duration. A bare mute
            // means "until my quiet hours end", and a machine that has not said
            // when those are has not said how long this mute lasts; picking a
            // length would be a mute the operator did not ask for, ending at an
            // hour they cannot predict.
            let Some(seconds) = until_quiet_ends else {
                return Err(NO_SCHEDULE.to_string());
            };
            Ok(QuietCommand::Mute {
                place: place.clone(),
                seconds,
            })
        }
        [place, word] => {
            if !known.iter().any(|name| name == place) {
                return Err(unmutable(place, known));
            }
            Ok(QuietCommand::Mute {
                place: place.clone(),
                seconds: crate::quiet::parse_duration(word)?,
            })
        }
        // ANY OTHER ARITY IS A REFUSAL, never a silent fallthrough to the
        // report: a typo the operator does not see is a mute they believe is
        // on.
        _ => Err(
            "pns: lights quiet takes a place, optionally with a duration or \
                  off, or nothing at all"
                .to_string(),
        ),
    }
}

/// Why a bare mute cannot be set on a machine with no quiet hours.
const NO_SCHEDULE: &str = "pns: lights quiet: a bare mute lasts until your quiet \
hours end, and `[plugins.hue] quiet_hours` states none; give a duration instead, \
or set that key";

/// Why one name cannot be muted, and what can be instead.
///
/// THE ALTERNATIVES ARE LISTED, because the name refused is often one the
/// operator is reading off their own config file or off the bridge's app, and
/// nothing on either page says which names a mute can reach. A refusal that
/// only repeats what was typed sends them back to whichever of the two misled
/// them.
fn unmutable(place: &str, known: &[String]) -> String {
    let reaches = if known.is_empty() {
        "this config claims no lamp at all, so there is nothing a mute could \
         reach"
            .to_string()
    } else {
        format!(
            "a mute reaches {}",
            known
                .iter()
                .map(|name| format!("{name:?}"))
                .collect::<Vec<String>>()
                .join(", ")
        )
    };
    format!(
        "pns: lights quiet: {place:?} is no lamp, room or zone this can quiet; \
         {reaches}"
    )
}

#[cfg(test)]
mod fixtures;

#[cfg(test)]
mod streak_tests;

#[cfg(test)]
mod unread_tests;

#[cfg(test)]
mod loop_tests;

#[cfg(test)]
mod phase_tests;

#[cfg(test)]
mod mute_tests;

#[cfg(test)]
mod quiet_command_tests;
