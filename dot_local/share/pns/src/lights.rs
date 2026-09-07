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
use pns_domain::lights::WORKING_SWEEP;
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

/// One `agent_status` per workspace, in the order herdr listed them, with a
/// workspace that carries no such field answering the EMPTY string.
///
/// A MISSING FIELD IS NOT A WORKING LOOP, which is the fail-toward-dark
/// direction this whole design takes, and it is not hypothetical: the suite's
/// own shipped herdr stub answers a `workspace list` with no `agent_status` in
/// it, and a herdr that stops carrying the field must leave a lamp dark rather
/// than breathing forever.
///
/// A SECOND READER OF ONE ANSWER, not a change to `parse_focused_tab`: that
/// function reads `focused` and `active_tab_id` for the visibility model and
/// has no business knowing what a lamp does.
pub fn workspace_agent_statuses(workspace_list_json: &str) -> Vec<String> {
    serde_json::from_str::<serde_json::Value>(workspace_list_json)
        .ok()
        .as_ref()
        .and_then(|body| body.pointer("/result/workspaces"))
        .and_then(serde_json::Value::as_array)
        .map(|workspaces| {
            workspaces
                .iter()
                .map(|workspace| {
                    workspace
                        .get("agent_status")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default()
                        .to_string()
                })
                .collect()
        })
        .unwrap_or_default()
}

/// One streak as one line: the two seconds, space separated, in
/// `render_heartbeat`'s shape.
pub fn render_streak(streak: &Streak) -> String {
    format!("{} {}", streak.since, streak.last_seen)
}

/// That line read back, or None for anything this will not vouch for.
///
/// REFUSED, NEVER GUESSED AT, in `parse_heartbeat`'s style. A file some other
/// hand rewrote is not a streak, and reading a garbled half as zero would
/// report a loop as having worked since 1970, which passes every threshold
/// there is and leaves a lamp breathing over nothing.
pub fn parse_streak(line: &str) -> Option<Streak> {
    let (since, last_seen) = line.trim_end_matches('\n').split_once(' ')?;
    Some(Streak {
        since: crate::parse_count(since)?,
        last_seen: crate::parse_count(last_seen)?,
    })
}

/// The record as one line: the two epochs, space separated, with `0` for a kind
/// that has not happened. `render_heartbeat`'s shape, and `render_streak`'s.
pub fn render_news(news: &News) -> String {
    format!(
        "{} {}",
        news.done_at.unwrap_or_default(),
        news.failed_at.unwrap_or_default()
    )
}

/// That line read back, or None for anything this will not vouch for.
///
/// REFUSED, NEVER GUESSED AT, in `parse_streak`'s style, and the fail direction
/// is DARK: a file some other hand rewrote yields no news, so the lamp stays
/// out rather than breathing about something nobody can name.
pub fn parse_news(line: &str) -> Option<News> {
    let (done, failed) = line.trim_end_matches('\n').split_once(' ')?;
    let epoch = |count: u64| (count > 0).then_some(count);
    Some(News {
        done_at: epoch(crate::parse_count(done)?),
        failed_at: epoch(crate::parse_count(failed)?),
    })
}

/// What the operator typed at `pns loop`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoopCommand {
    /// Take a lease for this pane, and hold the lamp until it is given back or
    /// times out.
    Begin(String),
    /// Give it back.
    End(String),
}

/// Where a pane's loop lease lives: one file per pane holding one epoch.
///
/// A DIRECTORY, LIKE THE WAITS, because several panes can each be running a
/// loop and each must be the only writer and the only ordinary remover of its
/// own file. One shared file would be a lease any other pane erases.
pub fn lease_dir(state_dir: &std::path::Path) -> std::path::PathBuf {
    state_dir.join("lights-loop")
}

/// One pane's lease path, or None for a pane id that cannot become a filename.
pub fn lease_marker(state_dir: &std::path::Path, pane: &str) -> Option<std::path::PathBuf> {
    crate::safety::pane_file_is_safe(pane).then(|| lease_dir(state_dir).join(pane))
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

/// One run's private name for a marker it has taken to remove.
pub fn sweep_claim(directory: &std::path::Path, name: &str, pid: u32) -> std::path::PathBuf {
    directory.join(format!("{name}{WORKING_SWEEP}{pid}"))
}

/// One held-record token, rendered: the bare path, or the path with its phase,
/// `@<end-unix-ms>:<brightness>:<state>`.
///
/// `@` AND `:` NEITHER APPEAR IN A FIXTURE PATH (`light/<id>` or
/// `grouped_light/<id>`, the id a bridge-issued UUID), and neither appears in
/// a state word, so the token round trips through the same whitespace-separated
/// line the bare record always used, with nothing to escape.
pub fn render_held_token(entry: &HeldEntry) -> String {
    match entry.resume {
        Some(phase) => format!(
            "{}@{}:{}:{}",
            entry.path,
            phase.end_unix_ms,
            phase.landed_on,
            phase.held.word()
        ),
        None => entry.path.clone(),
    }
}

/// One held-record token, parsed.
///
/// A MALFORMED SUFFIX IS NO PHASE, NEVER UNREADABLE: the record as a whole
/// already has an unreadable answer (`None`, held_lamps' own `Err(_)` arm) for
/// a file nothing here could open at all, and a token that arrived from an
/// older build, a hand edit, or a write this tick's own guard cut short is a
/// path this run still knows how to put out. Losing the phase costs one fade
/// of resume; inventing an unreadable path would cost the lamp.
pub fn parse_held_token(token: &str) -> HeldEntry {
    let Some((path, suffix)) = token.split_once('@') else {
        return HeldEntry::bare(token);
    };
    let phase = (|| {
        let (end_ms, rest) = suffix.split_once(':')?;
        let (landed_on, word) = rest.split_once(':')?;
        Some(Phase {
            end_unix_ms: end_ms.parse().ok()?,
            landed_on: landed_on.parse().ok()?,
            held: Held::from_word(word)?,
        })
    })();
    match phase {
        Some(resume) => HeldEntry {
            path: path.to_string(),
            resume: Some(resume),
        },
        None => HeldEntry::bare(path),
    }
}

/// Where the needs markers live: one file per waiting session.
pub fn blocked_dir(state_dir: &std::path::Path) -> std::path::PathBuf {
    state_dir.join("lights-blocked")
}

/// One session's marker path, or None for a session id that cannot become a
/// filename.
///
/// THE SESSION ID AND NOT THE PANE, and the difference is a path escape.
/// `pane_is_safe` permits `..` because a pane id becomes a shell WORD, never a
/// filename; `session_id_is_safe` forbids it and already backs a filename in
/// this same directory (`session-<id>.start`). Reusing it writes no new
/// predicate and opens no new door.
pub fn blocked_marker(state_dir: &std::path::Path, session_id: &str) -> Option<std::path::PathBuf> {
    crate::safety::session_id_is_safe(session_id).then(|| blocked_dir(state_dir).join(session_id))
}

/// The entries the state file holds, or ONE complaint naming what is wrong
/// with it.
///
/// IT REPORTS RATHER THAN GUESSES, and the fail DIRECTION is the caller's,
/// which is why it is not stated here: the two callers take opposite ones and
/// both are deliberate. `ad_hoc_quiet`, the lamp path, turns any complaint into
/// `Muting::Everything`, because a house with every lamp loud is the 3am the
/// mute was armed to prevent. `pns lights quiet`, the command, prints the
/// complaint and rebuilds from an empty list, because an operator standing in
/// front of it is losing what the file held and gets to see that rather than a
/// silent repair.
///
/// A LINE IS `<epoch> <place>` AND NOTHING ELSE, with the only leniency the ONE
/// trailing newline the publish itself writes. Padding is not something this
/// ever wrote, so a file carrying it was edited by something else: a `trim()`
/// here is exactly the leniency that read `" 9223372036854775807\n"` as a live
/// mute one module over.
///
/// THE PLACE IS THE REST OF THE LINE VERBATIM, spaces and all, because a room
/// is called `3F - Master Bedroom` and splitting on whitespace would make that
/// four fields. What it may not be is empty, or padded at either end, since
/// neither would ever match the name a family claims in.
pub fn muted_entries(contents: &str) -> Result<Vec<Muted>, String> {
    let held = contents.strip_suffix('\n').unwrap_or(contents);
    let lines: Vec<&str> = held.split('\n').collect();
    if lines.len() > MAX_MUTED_PLACES {
        return Err(quiet_state_error(format!(
            "{} lines, more than the {MAX_MUTED_PLACES} places it keeps",
            lines.len()
        )));
    }
    lines.iter().map(|line| muted_entry(line)).collect()
}

/// One line of it, or the complaint that quotes the line back.
fn muted_entry(line: &str) -> Result<Muted, String> {
    let refused = || quiet_state_error(format!("{line:?}, which is not an expiry and a place"));
    let (stated, place) = line.split_once(' ').ok_or_else(refused)?;
    if place.is_empty() || place.trim() != place {
        return Err(refused());
    }
    Ok(Muted {
        expiry: crate::parse_count(stated).ok_or_else(refused)?,
        place: place.to_string(),
    })
}

/// One wording for every way the file can be wrong, since the operator's move
/// is the same for all of them and a second sentence would only make two
/// problems look like one.
fn quiet_state_error(what: String) -> String {
    format!(
        "pns: state error (lights-quiet holds {what}); nothing is quiet, and \
         the next pns lights quiet write replaces the file"
    )
}

/// What the operator typed at `pns lights quiet`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuietCommand {
    /// No argument at all: say what is quiet and mute nothing. There is no
    /// untimed form, for `pns quiet`'s reason: a mute the operator forgets is
    /// a lamp that has silently stopped working.
    Report,
    Mute {
        place: String,
        seconds: u64,
    },
    Unmute {
        place: String,
    },
}

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

/// The file's body: one line per entry, in the order they are kept.
///
/// NO TRAILING NEWLINE, because `publish_state_line` writes one, and the parse
/// strips exactly that one. Two would read back as an empty last line, which
/// the parse refuses, so the round trip is what keeps this honest.
pub fn render_muted(entries: &[Muted]) -> String {
    entries
        .iter()
        .map(|entry| format!("{} {}", entry.expiry, entry.place))
        .collect::<Vec<String>>()
        .join("\n")
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
