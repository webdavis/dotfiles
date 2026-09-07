//! The pns binary: the composition root, and the only place with a main.
//!
//! Everything here is WIRING. The roster is one constant and one constructor
//! in `registry`, so there is no second construction of it to diverge; the
//! environment and the config are read once at this edge, and every decision
//! is delegated to the library. The producer path exits 0 on every path,
//! because a notification must never fail the work it reports on, and so
//! does `pns hook <event>` for every event but `blocked`, which, like
//! `pns gate`, passes through moshi's own exit code (see `moshi_decision`).
//! The hand-typed verbs refuse a bad invocation with exit 2, with two gaps
//! still open: `home` is a diagnostic that always exits 0, and a word
//! trailing `lights tick` is dropped rather than refused.

use std::collections::BTreeMap;
use std::io::{IsTerminal, Read, Seek, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use pns::args::parse_args;
use pns::channels::banner::BannerChannel;
use pns::channels::hermes::{
    DEFAULT_HERMES_URL, HermesChannel, UreqSignedPost, channel_url, hermes_secret, remote_deadline,
};
use pns::channels::hue::{
    BRIDGE_DEADLINE, HuePulse, TYPED_COMMAND_DEADLINE, UreqBridge, hue_settings, quiet_now,
    quiet_window,
};
use pns::channels::moshi::{
    DEFAULT_MOSHI_URL, MOSHI_TYPE, MoshiChannel, UreqPost, mobile_backend, moshi_secret,
    refused_backend_line,
};
use pns::channels::{Delivery, native_first};
use pns::config::{LoadOutcome, config_path, load_config};
use pns::engine::{Overrides, decide};
use pns::hooks::{
    HookPayload, condenser_prompt, condenser_verdict, flattened, moshi_subcommand, parse_payload,
    transcript_reply,
};
use pns::registry::{roster, select_plugins};
use pns::render;
use pns::system::{
    PROBE_READ_MAX, SystemCommandRunner, SystemProbes, local_minutes_since_midnight, run_bounded,
};

mod blocked_wait_markers;
mod channel_dispatch;
mod channel_settings;
mod command_daemon;
mod command_doctor;
mod command_home;
mod command_lights;
mod command_loop;
mod command_nag;
mod command_presence;
mod command_pulse;
mod command_recap;
mod daemon_child_runtime;
mod daemon_runtime;
mod daemon_spool_runtime;
mod doctor_reports;
mod event_flow;
mod event_records;
mod focus_runtime;
mod home_staleness_memory;
mod hook_dispatch;
mod hook_observations;
mod hook_payload;
mod invocation;
mod journal_claims;
mod lamp_diagnostics;
mod lamp_event_lease;
mod lamp_pulse;
mod lights_breath_runtime;
mod lights_house_runtime;
mod lights_marker_runtime;
mod lights_state_runtime;
mod lights_tick_runtime;
mod lights_tick_writes;
mod moshi_submission;
mod nag_schedule_runtime;
mod presence_runtime;
mod recap_delivery_runtime;
mod recap_sources_runtime;
mod return_replay;
mod return_window;
mod runtime_environment;
mod state_lock_runtime;
mod state_rings;
mod turn_condenser;
mod turn_lifecycle;
mod turn_text;

#[cfg(test)]
mod runtime_test_support;

pub(crate) use blocked_wait_markers::{end_blocked_wait, update_blocked_marker};
pub(crate) use channel_dispatch::dispatch_legs;
pub(crate) use channel_settings::{
    Mobile, disabled_backend_warnings, enabled_hue_table, plugin_settings, read_mobile,
};
pub(crate) use command_daemon::{DAEMON_USAGE, daemon_mode};
pub(crate) use command_doctor::doctor_mode;
pub(crate) use command_home::home_mode;
pub(crate) use command_lights::{LIGHTS_QUIET_SAID, ad_hoc_quiet, lights_mode};
pub(crate) use command_loop::{loop_mode, renew_loop_lease};
pub(crate) use command_nag::nag_mode;
pub(crate) use command_presence::{ensure_presence_poll, presence_mode, presence_settings};
pub(crate) use command_pulse::pulse_mode;
pub(crate) use command_recap::recap_mode;
pub(crate) use daemon_child_runtime::{Bounded, child_bound, reap, spawn_job};
pub(crate) use daemon_runtime::daemon_run;
pub(crate) use daemon_spool_runtime::drain_spool;
pub(crate) use doctor_reports::{daemon_line, decision_section, missed_line, read_pairing};
pub(crate) use event_flow::{Attempt, run_event};
pub(crate) use event_records::{
    DECISIONS, MISSED_NOTIFICATIONS, activity_in, record_activity, record_decision, record_missed,
};
pub(crate) use focus_runtime::{focus_line, focus_now};
pub(crate) use home_staleness_memory::{remember_staleness, remembered_staleness};
pub(crate) use hook_dispatch::hook_mode;
pub(crate) use hook_observations::{
    arm_quota_stale_wait, config_change_detail, model_switch_detail, quota_observation_detail,
    record_policy_settings_change,
};
pub(crate) use hook_payload::{payload_is_whole, read_payload};
pub(crate) use invocation::{USAGE, event_mode, is_producer_argv, second_argument};
pub(crate) use journal_claims::{claim_journal, owner_is_gone};
pub(crate) use lamp_diagnostics::{hue_resolves, lights_report, pulse_outcome};
pub(crate) use lamp_event_lease::{
    LIGHTS_JOB, ORDINARY_LEASE_SECS, clear_held_lamps, register_lights_tick, schedule_lights_tick,
};
pub(crate) use lamp_pulse::{fire_pulse, fire_pulse_unless_quiet, routing_complaints};
pub(crate) use lights_breath_runtime::{Breathing, drive_breaths};
pub(crate) use lights_house_runtime::lights_house;
pub(crate) use lights_marker_runtime::{
    blocked_lamp, sweep_leases, sweep_legacy_state, sweep_shell_markers,
};
pub(crate) use lights_state_runtime::{
    held_lamps, read_held, read_news, record_news, remember_held, say_lights_once,
};
pub(crate) use lights_tick_runtime::lights_tick;
pub(crate) use lights_tick_writes::{run_tick_writes, tick_bridge_deadline};
pub(crate) use moshi_submission::{blocking_event, gate_mode, moshi_hook_bin};
pub(crate) use nag_schedule_runtime::{
    BLOCKED_STATE, NAG_OFF, arm_nag, clear_nag, marker_path, nag_after_secs, write_marker,
};
pub(crate) use presence_runtime::{
    home_presence, last_narrowing, narrow_to_presence, presence_snapshot, presence_status,
    system_probes,
};
pub(crate) use recap_delivery_runtime::post_recap;
pub(crate) use recap_sources_runtime::{
    found, merged_pull_requests, modified_at, notes_matching, read_sources, truncated,
};
pub(crate) use return_replay::replay_missed;
pub(crate) use return_window::{Moment, claim_moment, mark_present, read_epoch};
pub(crate) use runtime_environment::{
    env_deadline, executable_in_path, now_secs, overrides_from_env, resolve_path, state_dir,
};
pub(crate) use state_lock_runtime::{HeldLock, claim_lock};
pub(crate) use state_rings::{
    RING_READ_MAX, STATE_FILE_MODE, append_ring_line, publish_state_line,
};
pub(crate) use turn_condenser::condense;
pub(crate) use turn_lifecycle::{end_of_turn, failed_turn, project_of, start_of_turn};
pub(crate) use turn_text::turn_reply;

#[cfg(test)]
pub(crate) use command_presence::{Polled, write_presence_reading};
#[cfg(test)]
pub(crate) use lights_state_runtime::LIGHTS_HELD;
#[cfg(test)]
pub(crate) use lights_tick_runtime::LIGHTS_SAID;

fn main() {
    // ONE READ OF ARGV, lossy rather than validating: `std::env::args()`
    // panics on non-UTF-8, and a stray byte degrading into an unknown token
    // (which the lenient parser already skips) is the honest failure mode
    // for an always-exit-0 notification path. `first`, the producer check
    // and the event parse each used to read `std::env::args_os()` on their
    // own; this is the one collection they share now.
    let argv: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect();
    let first = argv.first().cloned().unwrap_or_default();
    // The pulse is a MODE, not a leg: it fires on a long command's exit code
    // rather than on an event, so it leaves before any of the event wiring.
    if first == "pulse" {
        std::process::exit(pulse_mode());
    }
    // The home diagnostic: one reading of the router, said out loud. The
    // doctor mode (P3) will absorb it; until then this is how the probe is
    // drilled and how a wrong config is diagnosed.
    if first == "home" {
        home_mode();
        return;
    }
    // The operator's mute, typed and timed. Also a MODE: it writes the state
    // the event path reads, and delivers nothing itself.
    if first == "quiet" {
        std::process::exit(quiet_mode());
    }
    // One test send through every configured channel, and one line per
    // registered plugin about it. A MODE for the same reason the others are:
    // it takes no decision, so nothing about an event's plan reaches it.
    if first == "doctor" {
        std::process::exit(doctor_mode());
    }
    // The return recap, rendered from the activity ring and posted to Discord.
    // A MODE for the reason the others are: it takes no decision, so no event's
    // plan reaches it. The event path starts it detached; an operator can also
    // run it by hand, which is how it is drilled.
    if first == "recap" {
        std::process::exit(recap_mode());
    }
    // The clock. A MODE for the reason the others are: `run` takes no event
    // and delivers nothing itself, and the two typed verbs beside it only move
    // a file. Nothing on the event path below reaches it, and nothing here
    // reaches the event path except by re-executing this binary.
    if first == "daemon" {
        std::process::exit(daemon_mode(&second_argument()));
    }
    // The lamps' upkeep. A MODE beside the daemon's for the same reason: it
    // takes no decision and delivers nothing, and the daemon is what runs it.
    // It reaches the event path through nothing at all.
    if first == "lights" {
        std::process::exit(lights_mode(&second_argument()));
    }
    // The room sensor's own upkeep. A MODE beside the lamps' for the same
    // reason: it reads the bridge, publishes one state line and delivers
    // nothing, and the daemon is what runs it.
    if first == "presence" {
        std::process::exit(presence_mode(&second_argument()));
    }
    // The loop lease, taken and given back by hand. A MODE beside the lamps'
    // for the same reason: it moves one file and delivers nothing.
    if first == "loop" {
        std::process::exit(loop_mode(&second_argument()));
    }
    // The nudge about an approval nobody answered. A MODE for the reason the
    // others are: it takes no decision from an event and reads no stdin. It
    // takes NO SESSION ARGUMENT either, because coalescing means it looks at
    // every outstanding record rather than at the one whose timer woke it, so
    // an argument would be a value it had to ignore.
    if first == "nag" {
        std::process::exit(nag_mode());
    }
    // The first-run walk. A MODE that has to be reachable with NO CONFIG AT
    // ALL, which is the state it exists to end, and that is why it sits above
    // everything that loads one. Nothing on the event path reaches it and it
    // reaches nothing there: it asks questions, composes text and publishes a
    // file, and delivers nothing.
    if first == "setup" {
        std::process::exit(setup_mode());
    }
    // The gate moshi's OWN extension calls. pi and omp spawn
    // `helperBinary pi-hook`, and that field holds one PATHNAME with no room
    // for a subcommand, so the binary answers the bare harness word itself.
    if pns::hooks::is_harness_subcommand(&first) {
        std::process::exit(gate_mode(&first));
    }
    // The same gate, spelled the way an operator reads it. Both forms end in
    // gate_mode, which REFUSES a word it will not vouch for: falling through
    // to the event path instead is how the documented spelling used to fire a
    // notification about an empty event.
    if first == "gate" {
        std::process::exit(gate_mode(&second_argument()));
    }
    if first == "hook" {
        std::process::exit(hook_mode(&second_argument()));
    }
    // A WORD THAT NAMES NO COMMAND IS A TYPO, never an event. It is the house
    // rule `pns nag` and `pns lights` already keep, moved up to where argv[1]
    // is decided: the producer parser is deliberately lenient about a token it
    // does not know, so `pns stpo` used to skip the word, render an empty event
    // and deliver it. The always-exit-0 contract governs EVENT deliveries, and
    // a word naming no command never becomes one, so refusing it here
    // contradicts nothing. `--help`/`-h` still reaches `event_mode` from here
    // (see `is_producer_argv`): that parser holds the one help arm now, so
    // there is no second copy of it up here to answer help before anything
    // else runs.
    if !is_producer_argv(&argv) {
        eprint!("{USAGE}");
        std::process::exit(2);
    }
    event_mode(&argv);
}

/// The first ancestor of `path` that exists in its own right but resolves to
/// nothing, and why it does not resolve.
///
/// WHAT THIS IS FOR: `NotFound` at the config path is not proof the config is
/// absent. A dangling link ANYWHERE ABOVE it (`~/.config/pns` naming a
/// directory that was moved or never created) fails the leaf's own stat with
/// ENOENT, exactly as a genuinely missing config does. Told apart nowhere,
/// that reading walks the whole questionnaire and only fails at publication,
/// with every answer already typed and every secret already handed over.
///
/// IT CLIMBS ONLY AS FAR AS THE FIRST COMPONENT THAT EXISTS. Above that
/// everything resolves by definition, and below it the components really are
/// missing, which is the ordinary first run this must not refuse.
fn unresolvable_ancestor(path: &Path) -> Option<(PathBuf, std::io::Error)> {
    // `skip(1)`: `path` ITSELF has already been stated by the caller, and it
    // is the leaf's own `NotFound` that brought us here.
    for ancestor in path.ancestors().skip(1) {
        match ancestor.symlink_metadata() {
            // NOT THERE AS A NAME AT ALL: keep climbing. The component under
            // it is genuinely missing rather than broken.
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            // UNREADABLE, NOT ABSENT: refuse by the same rule the leaf's own
            // non-NotFound arm refuses under.
            Err(error) => return Some((ancestor.to_path_buf(), error)),
            // A NAME IS STANDING HERE. Whether it LEADS anywhere is the whole
            // question: `metadata` follows the link `symlink_metadata` did
            // not, so a dangling one (or a loop, or a file where a directory
            // belongs) answers with its own cause here.
            Ok(_) => {
                return match ancestor.metadata() {
                    Ok(_) => None,
                    Err(error) => Some((ancestor.to_path_buf(), error)),
                };
            }
        }
    }
    None
}

/// The `setup` mode: the first-run walk, and the only writer of the config.
///
/// A THIN EDGE OVER A PURE COMPOSER. Everything about what lands in the file
/// is `pns::setup`; this asks, reads a line, and publishes. It EXITS NON-ZERO
/// on every refusal, which the always-exit-0 contract permits for the same
/// reason `quiet` does: that contract covers the hook and notification paths,
/// where a non-zero exit fails the turn being reported on, and this is hand
/// typed and is never a hook.
///
/// IT REFUSES A WALK NOBODY CAN ANSWER. Without a terminal there is no walk,
/// and guessing every answer would write a config the operator never agreed
/// to, over one they may already have.
fn setup_mode() -> i32 {
    let arguments: Vec<String> = std::env::args_os()
        .skip(2)
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect();
    let force = match arguments.as_slice() {
        [] => false,
        [word] if word == "--force" => true,
        // ANY OTHER WORD IS A REFUSAL, never a silent fallthrough to the walk:
        // a mistyped `--force` that walked anyway would ask ten questions and
        // then refuse at the end, over a config it was told to replace.
        _ => {
            eprintln!("{SETUP_USAGE}");
            return 2;
        }
    };
    // AN EMPTY HOME IS REFUSED BY NAME, before the config is even located: an
    // unset or empty HOME would otherwise compose a config path relative to
    // the current directory, which is not the operator's own machine-wide
    // config no matter where this happened to be run from.
    let Some(home) = std::env::var("HOME").ok().filter(|home| !home.is_empty()) else {
        eprintln!("pns setup: HOME is unset or empty; nothing was written");
        return 2;
    };
    // THE CONFIG IS CHECKED BEFORE THE TERMINAL IS, because it is the more
    // specific answer: an operator who already has one is told that, whether
    // or not they are sitting in front of the questions.
    let path = config_path(&home);
    // `symlink_metadata`, NOT `exists`: `exists` follows a symlink and asks
    // what it resolves to, so a dangling one at the config name reads as
    // nothing at all here and the whole walk runs before the publish refuses
    // it with a claim that it "appeared while the questions were being
    // answered", which would not be true.
    match path.symlink_metadata() {
        Ok(_) if !force => {
            eprintln!(
                "pns setup: {} already exists; pass --force to replace it, \
                 which keeps the old file beside it",
                path.display()
            );
            return 2;
        }
        Ok(_) => {}
        // NOTHING AT THE NAME IS NOT YET NOTHING IN THE WAY: a dangling link
        // above the config reports `NotFound` here too, and it refuses
        // REGARDLESS OF `--force`, because what `--force` agrees to replace
        // is a config, not a path that leads nowhere.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if let Some((ancestor, cause)) = unresolvable_ancestor(&path) {
                eprintln!(
                    "pns setup: {} could not be checked: {} does not resolve ({cause}); \
                     nothing was written",
                    path.display(),
                    ancestor.display()
                );
                return 2;
            }
        }
        // ANY OTHER ERROR REFUSES REGARDLESS OF --force: the comment above
        // only holds for NotFound, and a directory this walk cannot even
        // stat is not one it can safely publish into either.
        Err(error) => {
            eprintln!(
                "pns setup: {} could not be checked: {error}; nothing was written",
                path.display()
            );
            return 2;
        }
    }
    if !std::io::stdin().is_terminal() {
        eprintln!(
            "pns setup: this is a walk through questions and stdin is not a terminal; \
             nothing was written"
        );
        return 2;
    }
    let answers = match walk() {
        Ok(answers) => answers,
        Err(reason) => {
            eprintln!("pns setup: {reason}; nothing was written");
            return 2;
        }
    };
    let composed = pns::setup::compose_config(&answers);
    // THROUGH THE ENGINE'S OWN PARSER BEFORE IT IS PUBLISHED. A wizard that
    // writes a config pns then refuses is worse than no wizard: it leaves a
    // machine falling back to the core with a complaint nobody is standing in
    // front of, and it does it while the operator is being told it worked.
    if let Err(error) = pns::config::parse_config(&composed) {
        eprintln!(
            "pns setup: what it composed does not load ({}); nothing was written",
            error.detail()
        );
        return 2;
    }
    match publish_config(&path, &composed, force) {
        Ok(backup) => {
            if let Some(backup) = backup {
                println!("pns setup: kept the old config at {}", backup.display());
            }
            println!("pns setup: wrote {}", path.display());
            0
        }
        Err(refusal) => {
            eprintln!("pns setup: {refusal}");
            1
        }
    }
}

/// The walk itself: one question at a time, in the order the file is written.
///
/// NONE OF THIS DECIDES ANYTHING. Every answer is carried to the composer as
/// it was typed, and a blank one is what declines a feature there. An `Err`
/// is the walk ending mid-conversation, named by its own reason, which
/// publishes nothing at all rather than composing a file out of half of one.
///
/// THE CREDENTIALS ARE ASKED INSIDE THE WALK, right after the feature they
/// arm, because a feature switched on now and credentialed later is exactly
/// the empty-value config this wizard exists to avoid.
fn walk() -> Result<pns::setup::Answers, String> {
    println!("{SETUP_PREAMBLE}");
    let mut answers = pns::setup::Answers {
        mobile_token: ask_hidden(
            "The phone card is on. Paste moshi's webhook secret to complete it, \
             or press enter to pair later",
        )?,
        ..Default::default()
    };

    if ask_yes("Post every event to hermes, for the durable log and the recap?")? {
        answers.hermes_key = armed_secret("hermes", "the signing key that route verifies")?;
    }
    if ask_yes("Flash hue lights green when work finishes and red when it dies?")? {
        // EACH ANSWER GATES THE NEXT QUESTION: once one comes back empty the
        // feature is already declined, and the rest would be questions whose
        // answers are thrown away.
        answers.hue_bridge = armed("the light pulse", "the hue bridge's address on the network")?;
        if !answers.hue_bridge.is_empty() {
            answers.hue_key = armed_secret("the light pulse", "an API key the bridge issued")?;
        }
        if !answers.hue_key.is_empty() {
            answers.hue_rooms = list(armed(
                "the light pulse",
                "the rooms to flash, comma separated, spelled as the bridge spells them",
            )?);
        }
    }
    if ask_yes("Read whether your phone is on the home wifi, off the router's client list?")? {
        // THE BACKEND HAS A WORKING DEFAULT and every other field here does
        // not, so this is the one question enter answers rather than declines.
        // A NAME NOTHING ANSWERS DECLINES THE PROBE, said here and not only in
        // the file: the composer writes that answer's table commented out, and
        // an operator who typed their router's brand deserves to hear why.
        match router_backend(&ask(&format!(
            "Which router backend? [{}]",
            pns::home::UNIFI_TYPE
        ))?) {
            None => println!(
                "  nothing here reads that router, so the home probe stays off; \
                 the file says how to arm it"
            ),
            Some(backend) => {
                answers.router_type = backend.to_string();
                answers.router_url = armed("the home probe", "the router's URL")?;
                if !answers.router_url.is_empty() {
                    answers.router_api_key =
                        armed_secret("the home probe", "an API key the router issued")?;
                }
                if !answers.router_api_key.is_empty() {
                    answers.router_device_hostname =
                        armed("the home probe", "the phone's hostname on that router")?;
                }
            }
        }
    }
    if ask_yes("Hold notifications back while a macOS Focus is on?")? {
        answers.focus_modes = list(armed(
            "focus silencing",
            "which Focus modes mean it, comma separated",
        )?);
    }
    answers.nag = ask_yes("Card you a second time about an approval left unanswered?")?;
    Ok(answers)
}

/// One credentialed answer, and the line that says what a blank one costs.
///
/// SAID WHEN IT HAPPENS rather than only in the file: an operator who meant to
/// arm a feature and pressed enter has one chance to notice, and the composed
/// file's own commented block is read later if at all.
fn armed(feature: &str, wanted: &str) -> Result<String, String> {
    Ok(nothing_given(feature, ask(wanted)?))
}

/// The same shape as `armed`, for a secret: read with the terminal's echo
/// held off, because this is where the token, the hermes key, the hue key
/// and the router key are all asked.
fn armed_secret(feature: &str, wanted: &str) -> Result<String, String> {
    Ok(nothing_given(feature, ask_hidden(wanted)?))
}

/// What `armed` and `armed_secret` share: the line a blank answer costs.
fn nothing_given(feature: &str, answer: String) -> String {
    if answer.is_empty() {
        println!("  nothing given, so {feature} stays off; the file says how to arm it");
    }
    answer
}

/// One question, and the line typed back. An `Err` names why nothing did: the
/// input ending and a read failing are different reasons, and this walk asks
/// for pasted answers, so a byte that is not valid UTF-8 is not a rare guest.
fn ask(question: &str) -> Result<String, String> {
    print!("{question}: ");
    let _ = std::io::stdout().flush();
    read_answer()
}

/// The same question, answered with the terminal's echo held off so a typed
/// secret never reaches the pane grid, herdr's persisted pane history, or any
/// attached client. THE GUARD ARMS BEFORE THE PROMPT PRINTS: arming after
/// would leave a window in which the prompt is already visible but echo is
/// still on, so an operator who types ahead of it, or this crate's own pty
/// test, could still have a secret echoed before `TCSAFLUSH` takes hold.
///
/// ONE CLIENT IS OUTSIDE THIS GUARD'S REACH: mosh, the transport under a
/// Moshi-connected phone, predicts keystrokes locally and can draw them on
/// that client transiently, ahead of the terminal's own echo state. Nothing
/// here controls that.
///
/// Ctrl-C, Ctrl-\, Ctrl-Z, a TERM or HUP, an alarm, and the two tty-stop
/// signals a backgrounded read raises are all held for the read rather than
/// answered immediately, the same trade `readpassphrase(3)` makes: each is
/// still delivered, just not until the guard drops, so Ctrl-C takes effect at
/// the next Enter rather than instantly.
fn ask_hidden(question: &str) -> Result<String, String> {
    let _hushed = Hushed::arm()?;
    print!("{question}: ");
    let _ = std::io::stdout().flush();
    read_answer()
}

/// What every read shares, hidden or not.
fn read_answer() -> Result<String, String> {
    let mut typed = String::new();
    match std::io::stdin().read_line(&mut typed) {
        Ok(0) => Err("the answers ended before the walk did".to_string()),
        Err(error) => Err(read_failure(&error, reading_from_the_background())),
        Ok(_) => Ok(answered(&typed)),
    }
}

/// Whether stdin's terminal is currently owned by some OTHER process group.
///
/// A FAILED `tcgetpgrp` IS NOT THIS CASE: a terminal that hung up answers -1
/// as well, and a read that failed on a dead terminal really did fail for its
/// own reason. A zero is no foreground group at all, which is not this either.
fn reading_from_the_background() -> bool {
    let foreground = unsafe { libc::tcgetpgrp(libc::STDIN_FILENO) };
    foreground > 0 && foreground != unsafe { libc::getpgrp() }
}

/// Why a read failed, in terms the operator can act on.
///
/// EIO FROM A BACKGROUND JOB IS NOT AN I/O FAULT, it is job control. The
/// hidden read blocks SIGTTIN, which is the set `readpassphrase(3)` holds and
/// what stops a suspension from stranding the terminal echo-off. termios(4)
/// names the trade directly: a background process that blocks or ignores
/// SIGTTIN gets `EIO` from the read "and no signal is sent", where an
/// unblocked one would have been stopped and could be resumed with `fg`.
///
/// Passed straight through, `pns setup &` therefore refuses with "Input/output
/// error", which names the symptom and hides the only thing the operator can
/// do about it. BOTH HALVES ARE REQUIRED: a bare EIO on a hung-up terminal is
/// a real failure, and a non-EIO error from the background (a non-UTF-8 paste,
/// say) still has its own honest reason to give.
fn read_failure(error: &std::io::Error, in_background: bool) -> String {
    if in_background && error.raw_os_error() == Some(libc::EIO) {
        return "this walk cannot read the terminal from the background; \
                bring it to the foreground with fg"
            .to_string();
    }
    format!("the answers could not be read: {error}")
}

/// Turns the terminal's echo off for as long as it lives. `Drop` restores
/// both the termios state and the signal mask it holds, on every exit path
/// including EOF and an unwinding panic: this crate carries no
/// `panic = "abort"`, so Drop always runs. Arming and the restore both apply
/// `TCSAFLUSH`, which also discards whatever was already queued, so a secret
/// typed ahead of its own prompt is lost rather than read, and so is an
/// answer typed ahead of the question after it.
struct Hushed {
    original: libc::termios,
    original_mask: libc::sigset_t,
}

impl Hushed {
    /// Arm the guard. FAILS CLOSED: a termios or signal call this cannot
    /// complete is refused as loudly as a bad answer, rather than silently
    /// leaving echo on and asking for a secret anyway.
    fn arm() -> Result<Hushed, String> {
        let mut original: libc::termios = unsafe { std::mem::zeroed() };
        if unsafe { libc::tcgetattr(libc::STDIN_FILENO, &mut original) } != 0 {
            return Err(format!(
                "the terminal's settings could not be read (tcgetattr: {})",
                std::io::Error::last_os_error()
            ));
        }
        let mut blocked: libc::sigset_t = unsafe { std::mem::zeroed() };
        if unsafe { libc::sigemptyset(&mut blocked) } != 0 {
            return Err(format!(
                "the signal mask could not be built (sigemptyset: {})",
                std::io::Error::last_os_error()
            ));
        }
        // BLOCKED FOR THE READ, not disabled: each one is still delivered,
        // once the guard drops and the mask is restored. THIS IS THE WHOLE
        // SET `readpassphrase(3)` HOLDS, all nine of them, because the doc
        // comment above cites that function as the model and a quietly
        // shorter set is the model's holes without its name. SIGTTIN and
        // SIGTTOU: a read that becomes a background job would otherwise be
        // stopped by SIGTTIN with echo still off, and Drop's own
        // `tcsetattr` from a background group can raise SIGTTOU before it
        // gets the chance to restore. SIGALRM: an alarm armed before the
        // walk began would otherwise end the process mid-prompt, and a
        // process that dies before `Drop` leaves the operator's terminal
        // echo-off with no prompt in front of it. SIGPIPE is inert today
        // (the Rust runtime sets it to `SIG_IGN` before `main`, so it ends
        // nothing to begin with) and is held anyway, so this set does not
        // have to be re-argued against the manual page every time the
        // runtime's own default moves.
        for signal in [
            libc::SIGINT,
            libc::SIGQUIT,
            libc::SIGTSTP,
            libc::SIGTERM,
            libc::SIGHUP,
            libc::SIGTTIN,
            libc::SIGTTOU,
            libc::SIGALRM,
            libc::SIGPIPE,
        ] {
            if unsafe { libc::sigaddset(&mut blocked, signal) } != 0 {
                return Err(format!(
                    "the signal mask could not be built (sigaddset: {})",
                    std::io::Error::last_os_error()
                ));
            }
        }
        let mut original_mask: libc::sigset_t = unsafe { std::mem::zeroed() };
        // `pthread_sigmask` IS POSIX, NOT BSD `errno`-STYLE: it RETURNS its
        // error number directly rather than setting errno, so the result
        // itself, not `last_os_error()`, is the only honest source for one.
        let masked =
            unsafe { libc::pthread_sigmask(libc::SIG_BLOCK, &blocked, &mut original_mask) };
        if masked != 0 {
            return Err(format!(
                "signals could not be held for the read (pthread_sigmask: {})",
                std::io::Error::from_raw_os_error(masked)
            ));
        }
        let mut hushed = original;
        hushed.c_lflag &= !libc::ECHO;
        hushed.c_lflag |= libc::ECHONL;
        if unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSAFLUSH, &hushed) } != 0 {
            let error = std::io::Error::last_os_error();
            unsafe {
                libc::pthread_sigmask(libc::SIG_SETMASK, &original_mask, std::ptr::null_mut());
            }
            return Err(format!(
                "the terminal's echo could not be turned off (tcsetattr: {error})"
            ));
        }
        Ok(Hushed {
            original,
            original_mask,
        })
    }
}

impl Drop for Hushed {
    fn drop(&mut self) {
        unsafe {
            // TERMIOS FIRST, THEN THE MASK: a signal delivered between the
            // two would otherwise run with the operator's terminal still
            // echo-off. Neither call's failure is checked: a tty that hung
            // up during the read (EOF from a closed pty) makes `tcsetattr`
            // fail, and Drop must never panic over a terminal already gone.
            libc::tcsetattr(libc::STDIN_FILENO, libc::TCSAFLUSH, &self.original);
            libc::pthread_sigmask(libc::SIG_SETMASK, &self.original_mask, std::ptr::null_mut());
        }
    }
}

/// What a typed line means as an answer.
///
/// A LINE OF NOTHING BUT SPACES IS A BLANK ONE, which is the rule the whole
/// walk rests on: `compose_config` declines a feature whose credential is
/// empty, and it asks `is_empty`, so a credential of two spaces would arm a
/// plugin with two spaces and deliver nothing while reading as set up. That is
/// the exact state this wizard exists to keep off a fresh machine, and the
/// trailing newline every line carries is what makes it reachable.
fn answered(line: &str) -> String {
    line.trim().to_string()
}

/// One yes-or-no question. ENTER MEANS NO, and so does anything that is not a
/// yes: this walk arms features that deliver to a phone and to lamps, and the
/// answer nobody typed on purpose must be the one that changes nothing.
fn ask_yes(question: &str) -> Result<bool, String> {
    Ok(means_yes(&ask(&format!("{question} [y/N]"))?))
}

/// Whether an answer to a yes-or-no question was a yes.
///
/// ONLY A YES IS ONE. Enter, a word nobody meant, and a mistyped `yes` all
/// mean no, because every question this answers arms something that delivers
/// to a phone or to a lamp and takes a credential to do it.
fn means_yes(answer: &str) -> bool {
    matches!(answer.to_lowercase().as_str(), "y" | "yes")
}

/// Which compiled-in backend an answer names, or `None` for one no backend
/// answers.
///
/// THE SET IS THE CODE'S, never a list kept here: `home` is what refuses a
/// type at probe time, so a wizard restating its own copy of that set would go
/// on accepting yesterday's answer the day a second backend lands. Enter names
/// the one there is, and a spelling that differs only in case is that one too,
/// written back as the code spells it rather than as it was typed.
fn router_backend(answer: &str) -> Option<&'static str> {
    (answer.is_empty() || answer.eq_ignore_ascii_case(pns::home::UNIFI_TYPE))
        .then_some(pns::home::UNIFI_TYPE)
}

/// A comma-separated answer as the values it names, blanks dropped.
fn list(answer: String) -> Vec<String> {
    answer
        .split(',')
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

/// Publish the composed config, keeping the old one when replacing it.
///
/// CREATE-IF-ABSENT, NEVER A BLANKET RENAME, on both paths: a config that
/// appeared between the check in `setup_mode` and this moment is another
/// writer's, and this run has not read it. The link failing with
/// `AlreadyExists` IS that refusal. NOTHING ASKS WHETHER A CONFIG IS THERE
/// either, because the answer stops being true the instant it is given: what
/// `--force` moves aside is the file it found at the name, and what it
/// publishes into is a name it emptied itself.
///
/// THE OLD CONFIG IS MOVED ASIDE RATHER THAN COPIED ASIDE, so the backup holds
/// what was actually replaced rather than what stood there when a copy ran, and
/// the old config is at one of the two names at every instant.
///
/// THE PENDING FILE CARRIES THE MODE, because it is what gets published:
/// writing at the umask would publish a config whose plugin secrets any
/// process on the machine can read.
fn publish_config(path: &Path, composed: &str, force: bool) -> Result<Option<PathBuf>, String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no directory to write in", path.display()))?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("{} could not be created: {error}", parent.display()))?;
    let pending = parent.join(pending_name());
    // CREATED OR NOT AT ALL, and never opened. A pending file is a second name
    // for the live config between the link that publishes it and the unlink
    // that removes it, so an abandoned run leaves one behind and process ids
    // are reused: an open that truncates would empty a config this run has not
    // read, and the backup taken next would hold the replacement.
    let file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(CONFIG_FILE_MODE)
        .open(&pending)
        .map_err(|error| format!("{} could not be written: {error}", pending.display()))?;
    let published = write_then_publish(path, &pending, file, composed, force);
    // WHICHEVER WAY IT WENT, and only ever the file the line above made: a
    // pending file left in the config directory would be read by nobody and
    // found by everybody, and removing one this run did not create is the
    // mirror of the write it refuses to do.
    let _ = std::fs::remove_file(&pending);
    published
}

/// The name the composed config is written under before it is published.
///
/// THE MOMENT AS WELL AS THE PROCESS, because the create above is exclusive: a
/// leftover from an abandoned run of the same id would otherwise refuse a
/// wizard nobody can unblock, and a name nothing else is holding is also a
/// name nothing else can be waiting at.
fn pending_name() -> String {
    format!(
        "config.toml.new.{}.{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since_epoch| since_epoch.subsec_nanos())
    )
}

/// The publish itself, with `publish_config` owning the cleanup around it.
fn write_then_publish(
    path: &Path,
    pending: &Path,
    mut file: std::fs::File,
    composed: &str,
    force: bool,
) -> Result<Option<PathBuf>, String> {
    // AND AGAIN AFTER THE OPEN, for `publish_state_line`'s reason: the mode an
    // open asks for is masked by the umask, and a config published without the
    // operator's own bits is one they cannot read.
    file.set_permissions(std::fs::Permissions::from_mode(CONFIG_FILE_MODE))
        .map_err(|error| format!("{} could not be secured: {error}", pending.display()))?;
    file.write_all(composed.as_bytes())
        .map_err(|error| format!("{} could not be written: {error}", pending.display()))?;

    // THE FORCED PATH EMPTIES THE NAME FIRST, and what it moves out of the way
    // is the backup. Nothing here asks whether a config is there: the move
    // itself is the answer, and it is the same answer a moment later.
    let kept = if force { keep_aside(path)? } else { None };
    // AND BOTH PATHS PUBLISH THE SAME WAY. A link that refuses an occupied
    // name cannot write over a config this run never read: after the dangling
    // symlink pre-check in `setup_mode`, the only way a config can be
    // standing here is a genuine arrival while the questions were being
    // answered, so "appeared" below is exact rather than one of two guesses.
    match std::fs::hard_link(pending, path) {
        Ok(()) => Ok(kept),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Err(format!(
            "{} appeared while the questions were being answered; \
             nothing was written over it{}",
            path.display(),
            also_kept(kept.as_deref())
        )),
        Err(error) => Err(format!(
            "{} could not be written: {error}{}",
            path.display(),
            also_kept(kept.as_deref())
        )),
    }
}

/// The tail a refusal carries when this run had already moved a config aside,
/// so nobody is left hunting for a file the wizard took the name of.
fn also_kept(kept: Option<&Path>) -> String {
    kept.map_or_else(String::new, |backup| {
        format!(
            "; the config that was there is kept at {}",
            backup.display()
        )
    })
}

/// Move the existing config aside, and answer with where it went.
///
/// A MOVE RATHER THAN A COPY, which is what makes the answer true: a copy says
/// only what stood at the name when the copy ran, and the publish that follows
/// replaces whatever stands there THEN. Moving it is the one act that both
/// keeps the old config and frees the name, so the two can never disagree.
///
/// NOTHING TO MOVE IS NOT A FAILURE: `--force` on a machine with no config is
/// an ordinary first run.
fn keep_aside(path: &Path) -> Result<Option<PathBuf>, String> {
    let now = now_secs().ok_or_else(|| {
        "the clock cannot be read, so the config already there cannot be named \
         and kept; nothing was written"
            .to_string()
    })?;
    keep_aside_at(path, now)
}

/// `keep_aside` with the moment NAMED rather than read.
///
/// THE SPLIT EXISTS FOR THE TEST, and the test is what makes it worth having.
/// With the clock read in here, a test that pre-claims a backup name has to
/// read the clock itself and hope neither read lands on the far side of a
/// second boundary. Pre-claiming both candidate names only narrows that
/// window: a thread parked across more than one boundary still picks a third
/// name and the test fails on a working build. Naming the second removes the
/// race instead of shrinking it.
fn keep_aside_at(path: &Path, epoch_secs: u64) -> Result<Option<PathBuf>, String> {
    let backup = pns::setup::backup_path(path, epoch_secs).ok_or_else(|| {
        format!(
            "{} cannot be named for keeping, so the config already there \
             cannot be kept; nothing was written",
            path.display()
        )
    })?;
    // THE NAME IS CLAIMED BEFORE ANYTHING MOVES ONTO IT, so a second forced run
    // inside the same second refuses rather than writing over the copy the
    // first one kept: a rename would replace that copy without a word.
    std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(CONFIG_FILE_MODE)
        .open(&backup)
        .map_err(|error| match error.kind() {
            // THE NAME BEING TAKEN PROVES NOTHING ABOUT WHAT IT HOLDS: a run
            // killed between this claim and the rename that follows it
            // leaves an empty file at the same name, so the refusal says
            // only that the name is spoken for, not what a prior run "kept"
            // there.
            std::io::ErrorKind::AlreadyExists => format!(
                "{} is already claimed by another run this same second; \
                 nothing was written",
                backup.display()
            ),
            // ANY OTHER FAILURE IS ITS OWN REASON: naming the same-second
            // collision for a permission refusal would blame a run that
            // never happened.
            _ => format!("{} could not be claimed: {error}", backup.display()),
        })?;
    if let Err(error) = std::fs::rename(path, &backup) {
        // THE CLAIM GOES WITH THE RUN THAT MADE IT, whether there was nothing
        // to move or the move could not be made: an empty file named like a
        // backup is worse than no backup at all.
        let _ = std::fs::remove_file(&backup);
        return match error.kind() {
            std::io::ErrorKind::NotFound => Ok(None),
            // THE BACKUP WAS NEVER THE PROBLEM HERE: it is a fresh file this
            // call just created, and what could not be moved onto it is
            // `path` itself, so the refusal names that instead.
            _ => Err(format!(
                "{} could not be moved aside to keep it: {error}",
                path.display()
            )),
        };
    }
    // AS PRIVATE AS THE CONFIG IT HOLDS, when what moved is a file at all: the
    // mode of a symlink is the mode of what it points at, and this one points
    // at a file this run did not replace and has no business changing.
    if backup.symlink_metadata().is_ok_and(|entry| entry.is_file()) {
        std::fs::set_permissions(&backup, std::fs::Permissions::from_mode(CONFIG_FILE_MODE))
            .map_err(|error| format!("{} could not be secured: {error}", backup.display()))?;
    }
    Ok(Some(backup))
}

/// The config carries every plugin's secret, so it is the operator's alone.
const CONFIG_FILE_MODE: u32 = 0o600;

/// What the walk says before it starts asking.
const SETUP_PREAMBLE: &str = "\
pns setup: a few questions, and a config at the end of them.
The macOS banner and the phone card are on and are not asked about. Everything
else is off unless you arm it here, and enter is no. Nothing is written until
the last answer.";

/// What a setup typed wrong is told.
const SETUP_USAGE: &str =
    "pns: usage: pns setup [--force]; --force replaces an existing config, keeping it beside";

/// The `quiet` mode: the operator's own mute, typed and timed.
///
/// THE ONLY NON-ZERO EXITS HERE THAT ARE NOT AN OPERATOR'S APPROVAL DECISION,
/// and they are correct. The always-exit-0 contract covers the hook and
/// notification paths, where a non-zero exit would fail the turn being
/// reported on; this is hand typed, is never a hook, and a subcommand that
/// silently swallows a typo is a mute the operator believes is on.
///
/// THE REPORT IS READ BACK OFF THE FILE after whatever was asked for, rather
/// than rendered from what this run intended, so the line cannot claim a mute
/// that never landed. A FAILED SET REPORTS TOO, for the mirror of the same
/// reason: it knows only that its own write did not happen, and a previous
/// mute may still be standing behind it.
fn quiet_mode() -> i32 {
    let arguments: Vec<String> = std::env::args_os()
        .skip(2)
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect();
    let quiet_until = state_dir().join(QUIET_UNTIL);
    // A SET THAT DID NOT HAPPEN, carried to the exit code rather than
    // returned on the spot, so the report below runs on this path too.
    let mut set_failed = false;
    match arguments.as_slice() {
        // NO ARGUMENT REPORTS and mutes nothing. There is no untimed toggle:
        // an indefinite mute the operator forgets is a notification system
        // that has silently stopped working, and making this form the report
        // also means no invocation can mute by accident.
        [] => {}
        // Unlinking is also how a file nothing can parse is cleared, which is
        // the remedy the corrupt-state complaint names.
        [word] if word == "off" => {
            let _ = std::fs::remove_file(&quiet_until);
        }
        [duration] => match pns::quiet::parse_duration(duration) {
            Ok(seconds) => {
                // NEITHER ARM CLAIMS "nothing is muted". A run that could not
                // read a clock or could not write cannot see the state it is
                // making a claim about, and a mute set an hour ago can be
                // standing behind both: measured, the write arm said nothing
                // was muted while `pns quiet` a second later reported sixty
                // minutes left. They say what did not happen, and the report
                // below says what stands.
                match now_secs().map(|now| now.saturating_add(seconds)) {
                    None => {
                        eprintln!(
                            "pns: state error (the clock cannot be read); the mute was not set"
                        );
                        set_failed = true;
                    }
                    // LOUD, unlike `remember_staleness`: that one is a
                    // background warning that must never crash a diagnostic,
                    // and this is a human waiting on an answer. Reporting
                    // success for a mute that is not in effect is the worst
                    // outcome available.
                    Some(expiry) => {
                        if let Err(error) = publish_state_line(&quiet_until, &expiry.to_string()) {
                            eprintln!(
                                "pns: state error (quiet-until could not be written: {error}); \
                                 the mute was not set"
                            );
                            set_failed = true;
                        }
                    }
                }
            }
            Err(refusal) => {
                eprintln!("{refusal}");
                eprintln!("{QUIET_USAGE}");
                return 2;
            }
        },
        // ANY EXTRA WORD IS A REFUSAL, never a silent fallthrough to the
        // report: a typo an operator does not see is a mute they believe is
        // on.
        _ => {
            eprintln!("{QUIET_USAGE}");
            return 2;
        }
    }
    println!(
        "{}",
        pns::quiet::status_line(read_quiet_expiry(), now_secs())
    );
    if set_failed { 1 } else { 0 }
}

/// What a mute typed wrong is told, once, on stderr. The refusal above it
/// quotes what was typed; this says what the command takes.
const QUIET_USAGE: &str =
    "pns: usage: pns quiet [<duration>|off]; duration is <count><s|m|h>, from 1s to 24h";

/// One line, holding the epoch second the operator's mute ends. ABSENT is the
/// ordinary state and the file is never created to say "not muted": every
/// reader compares the expiry with its own clock, so a file left behind after
/// the window is already inert.
const QUIET_UNTIL: &str = "quiet-until";

/// The mute's expiry, if the operator set one.
///
/// A FILE NOTHING CAN READ OR PARSE COMPLAINS AND READS AS NOT MUTED, which
/// is the OPPOSITE of the lights window's fail-closed reading and deliberately
/// so: a window failing closed costs one flash of a lamp, and a mute failing
/// closed costs every notification, including the card for a tool call the
/// operator is blocked on, with no expiry and no way for them to see it. The
/// complaint repeats for as long as the file stays broken, which is
/// proportional: it IS broken until someone fixes it.
///
/// ONLY AN ABSENT FILE IS SILENT, and it is the ordinary state. A single
/// `.ok()?` used to cover both, so a file that could not be read at all was
/// as quiet as one that was never there: unreadable permissions, a directory
/// standing in its place and bytes that are not UTF-8 each muted nothing and
/// announced nothing, which is the state nobody can discover.
fn read_quiet_expiry() -> Option<u64> {
    let raw = match std::fs::read_to_string(state_dir().join(QUIET_UNTIL)) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
        Err(error) => {
            eprintln!(
                "pns: state error (quiet-until could not be read: {error}); \
                 nothing is muted, clear it with pns quiet off"
            );
            return None;
        }
    };
    pns::quiet::expiry_from_state(&raw)
        .inspect_err(|complaint| eprintln!("{complaint}"))
        .ok()
}

/// Whether the operator's mute is on, judged on THE RUN'S OWN clock reading:
/// the same one the rest of the decision is taken against. An expiry crossed
/// mid-run costs one event either way, and one decision on one reading is the
/// engine's stated contract.
fn muted_now(now_secs: Option<u64>) -> bool {
    pns::quiet::is_muted(read_quiet_expiry(), now_secs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_test_support::*;
    #[test]
    fn the_only_answer_that_arms_a_feature_is_a_yes_somebody_typed() {
        // ENTER MEANS NO, and this is the assertion that says so. Every
        // question it answers arms a delivery to a phone or a lamp and takes a
        // credential to do it, so the answer nobody typed on purpose has to be
        // the one that changes nothing. A predicate reading "not a no" would
        // arm the whole walk by default and pass every test about the file.
        for yes in ["y", "yes", "Y", "YES", "Yes"] {
            assert!(means_yes(yes), "`{yes}` is a yes");
        }
        for no in ["", "n", "no", "N", "sure", "ok", "yeah", "yep", "y ", "1"] {
            assert!(!means_yes(no), "`{no}` is not the yes this walk requires");
        }
    }

    #[test]
    fn an_answer_of_nothing_but_spaces_is_a_blank_one() {
        // THE RULE THE WHOLE WALK RESTS ON. `compose_config` declines a
        // feature whose credential is empty and it asks `is_empty`, so a
        // credential that survives here as `"  "` arms its plugin with two
        // spaces: a table that reads as set up and delivers nothing, which is
        // the one state this wizard exists to keep off a fresh machine.
        assert_eq!(answered("   \n"), "");
        assert_eq!(answered("\t\n"), "");
        assert_eq!(answered("\n"), "");
        // AND A REAL ANSWER SURVIVES IT: trimming that ate the answer would
        // decline every feature the operator armed.
        assert_eq!(answered("  192.168.1.9  \n"), "192.168.1.9");
        assert_eq!(answered("Studio, Kitchen\n"), "Studio, Kitchen");
    }

    #[test]
    fn a_comma_separated_answer_names_only_the_values_somebody_typed() {
        // A BLANK BETWEEN TWO COMMAS IS NOT A ROOM. It would reach the file as
        // `rooms = [""]`, which the bridge matches to no room at all while the
        // table reads as configured.
        assert_eq!(list("Studio, Kitchen".to_string()), ["Studio", "Kitchen"]);
        assert_eq!(
            list("Studio, , Kitchen,".to_string()),
            ["Studio", "Kitchen"]
        );
        assert_eq!(list("  Studio  ".to_string()), ["Studio"]);
        assert!(list(String::new()).is_empty());
        assert!(list(" , ".to_string()).is_empty());
    }

    #[test]
    fn the_only_backend_the_walk_accepts_is_one_the_home_probe_answers() {
        // THE ONE QUESTION WHOSE ANSWER IS NOT FREE TEXT. Every other answer
        // here is a credential nothing but the operator's own network can
        // judge; this one is judged by `home`, which refuses a type it does
        // not implement at probe time, long after the wizard said it worked.
        assert_eq!(router_backend(""), Some(pns::home::UNIFI_TYPE));
        assert_eq!(router_backend("unifi"), Some(pns::home::UNIFI_TYPE));
        // AND THE ANSWER IS WRITTEN AS THE CODE SPELLS IT, because the probe
        // compares the whole string and would refuse the operator's capitals.
        assert_eq!(router_backend("UniFi"), Some(pns::home::UNIFI_TYPE));
        for unanswerable in ["asus", "unifi-controller", "u", "unifix", "eero"] {
            assert_eq!(
                router_backend(unanswerable),
                None,
                "`{unanswerable}` is a backend nothing here reads"
            );
        }
    }

    /// The mode a file was published with, and nothing else about it.
    fn mode_of(path: &std::path::Path) -> u32 {
        std::fs::metadata(path)
            .expect("the file")
            .permissions()
            .mode()
            & 0o777
    }

    /// Everything beside the published config in its directory: empty when a
    /// publish left no pending file and claimed no unclaimed backup name.
    fn leftovers(path: &std::path::Path) -> Vec<String> {
        std::fs::read_dir(path.parent().expect("the directory"))
            .expect("the directory")
            .filter_map(|entry| Some(entry.ok()?.file_name().to_string_lossy().into_owned()))
            .filter(|name| name != "config.toml")
            .collect()
    }

    #[test]
    fn a_first_config_is_published_for_its_operator_alone_and_leaves_no_pending_file() {
        // THE FILE CARRIES EVERY PLUGIN'S SECRET, so publishing it at the
        // umask hands the moshi token and the hue key to every process on the
        // machine. The pending file carries them too, which is why it is
        // created with the mode rather than chmodded into it afterwards, and
        // why it never outlives the publish.
        let home = scratch("setup-publish-first");
        let path = home.join(".config/pns/config.toml");
        assert_eq!(
            publish_config(&path, "# composed\n", false),
            Ok(None),
            "a first publish keeps nothing aside"
        );
        assert_eq!(
            std::fs::read_to_string(&path).expect("the config"),
            "# composed\n"
        );
        assert_eq!(
            mode_of(&path),
            CONFIG_FILE_MODE,
            "the config is the operator's alone"
        );
        let extra = leftovers(&path);
        assert!(
            extra.is_empty(),
            "a pending file was left behind: {extra:?}"
        );
    }

    #[test]
    fn a_config_that_appeared_during_the_walk_is_refused_rather_than_written_over() {
        // CREATE-IF-ABSENT, NEVER A BLANKET RENAME. The questions take
        // minutes, and a config that arrived while they were being answered is
        // another writer's: a rename would replace it with no backup and no
        // word, and the refusal earlier in `setup_mode` cannot see it because
        // it ran before the walk did.
        let home = scratch("setup-publish-raced");
        let path = home.join(".config/pns/config.toml");
        std::fs::create_dir_all(path.parent().expect("the directory")).expect("the directory");
        std::fs::write(&path, "# somebody else got here first\n").expect("the config");

        let refusal = publish_config(&path, "# composed\n", false).expect_err("it must refuse");
        assert!(
            refusal.contains("appeared"),
            "it says what happened: {refusal}"
        );
        assert_eq!(
            std::fs::read_to_string(&path).expect("the config"),
            "# somebody else got here first\n",
            "the config that was already there was written over"
        );
        let extra = leftovers(&path);
        assert!(extra.is_empty(), "a refusal left a pending file: {extra:?}");
    }

    #[test]
    fn a_forced_replacement_keeps_the_old_config_before_it_writes_the_new_one() {
        // THE BACKUP IS TAKEN FIRST, and the way to say that as an assertion
        // is to read the backup: taken afterwards it would be a copy of the
        // REPLACEMENT, the old file would be gone, and the line printed to the
        // operator would name a path that does not hold what it says it holds.
        let home = scratch("setup-publish-forced");
        let path = home.join(".config/pns/config.toml");
        std::fs::create_dir_all(path.parent().expect("the directory")).expect("the directory");
        std::fs::write(&path, "# the one it replaces\n").expect("the config");

        let backup = publish_config(&path, "# composed\n", true)
            .expect("a forced publish")
            .expect("it kept the old config");
        assert_eq!(
            std::fs::read_to_string(&path).expect("the config"),
            "# composed\n"
        );
        assert_eq!(
            std::fs::read_to_string(&backup).expect("the backup"),
            "# the one it replaces\n",
            "the backup holds the replacement rather than what was replaced"
        );
        // AND IT IS AS PRIVATE AS THE FILE IT COPIES: a backup of a config
        // full of plugin secrets is a config full of plugin secrets.
        assert_eq!(mode_of(&backup), CONFIG_FILE_MODE);
        assert!(
            !backup.to_string_lossy().contains(':'),
            "the stamp carries colons: {}",
            backup.display()
        );
    }

    #[test]
    fn a_forced_replacement_with_nothing_to_replace_keeps_nothing_aside() {
        // THE MIRROR: `--force` on a machine with no config is an ordinary
        // first run, and naming a backup that holds nothing would send the
        // operator to a file that was never written.
        let home = scratch("setup-publish-forced-first");
        let path = home.join(".config/pns/config.toml");
        assert_eq!(publish_config(&path, "# composed\n", true), Ok(None));
        assert_eq!(
            std::fs::read_to_string(&path).expect("the config"),
            "# composed\n"
        );
        assert_eq!(mode_of(&path), CONFIG_FILE_MODE);
        // AND IT LEAVES NO FILE NAMED LIKE ONE EITHER. Claiming the backup's
        // name is how a second forced run in the same second is refused, and a
        // claim left standing over nothing is a backup that holds nothing.
        let extra = leftovers(&path);
        assert!(extra.is_empty(), "it kept something aside: {extra:?}");
    }

    #[test]
    fn a_forced_run_keeps_a_config_the_existence_check_reads_as_absent() {
        // THE CHECK IS NOT THE AUTHORITY, THE PUBLISH IS. The walk's own
        // pre-check reads `symlink_metadata` rather than `exists`, so a
        // dangling symlink at the config name is refused before the first
        // question is even asked; this proves the FORCED publish handles the
        // same dangling symlink correctly on its own, which must not depend
        // on the pre-check having caught it. Either way a blanket rename
        // replaced a config this run never read, with no backup and no word,
        // so the publish moves aside whatever is standing there and asks for
        // the name rather than taking it.
        let home = scratch("setup-publish-unseen");
        let path = home.join(".config/pns/config.toml");
        std::fs::create_dir_all(path.parent().expect("the directory")).expect("the directory");
        let pointed_at = path.with_file_name("config-in-a-checkout.toml");
        std::os::unix::fs::symlink(&pointed_at, &path).expect("the link");

        let backup = publish_config(&path, "# composed\n", true)
            .expect("a forced publish")
            .expect("it kept the config that was standing there");
        assert_eq!(
            std::fs::read_link(&backup).expect("the backup"),
            pointed_at,
            "the config that was there went nowhere this run can name"
        );
        assert_eq!(
            std::fs::read_to_string(&path).expect("the config"),
            "# composed\n"
        );
    }

    #[test]
    fn a_forced_run_keeps_the_config_it_replaced_rather_than_what_that_config_named() {
        // WHAT THE BACKUP HOLDS IS WHAT THE PUBLISH REPLACED. A copy taken
        // from the name reads THROUGH it: with a symlinked config it copied
        // the file at the far end, which the publish then did not touch, and
        // the link itself, which the publish did replace, went unrecorded. The
        // same gap a config replaced between the copy and the publish leaves,
        // which no test can reach without a seam.
        let home = scratch("setup-publish-through");
        let path = home.join(".config/pns/config.toml");
        std::fs::create_dir_all(path.parent().expect("the directory")).expect("the directory");
        let pointed_at = path.with_file_name("config-in-a-checkout.toml");
        std::fs::write(&pointed_at, "# the one it points at\n").expect("the config");
        std::os::unix::fs::symlink(&pointed_at, &path).expect("the link");

        let backup = publish_config(&path, "# composed\n", true)
            .expect("a forced publish")
            .expect("it kept the config that was standing there");
        assert_eq!(
            std::fs::read_link(&backup).expect("the backup"),
            pointed_at,
            "the backup holds what the config named rather than the config it replaced"
        );
        // AND WHAT IT NAMED WAS NOT REPLACED, so it is where it always was.
        assert_eq!(
            std::fs::read_to_string(&pointed_at).expect("the config it points at"),
            "# the one it points at\n"
        );
        assert_eq!(
            std::fs::read_to_string(&path).expect("the config"),
            "# composed\n"
        );
    }

    #[test]
    fn a_pending_file_left_by_an_abandoned_run_is_never_the_file_this_one_writes_into() {
        // A PENDING FILE IS A SECOND NAME FOR THE LIVE CONFIG between the link
        // that publishes it and the unlink that removes it, so a run killed in
        // that window leaves one behind. PROCESS IDS ARE REUSED, so a later
        // run naming its pending file after its own id can find that leftover,
        // and opening it to truncate would empty the config this run has not
        // read yet: the backup taken next would hold the REPLACEMENT, under a
        // path printed to the operator as the file they had.
        let home = scratch("setup-publish-leftover");
        let path = home.join(".config/pns/config.toml");
        std::fs::create_dir_all(path.parent().expect("the directory")).expect("the directory");
        std::fs::write(&path, "# the one it replaces\n").expect("the config");
        let leftover = path.with_file_name(format!("config.toml.new.{}", std::process::id()));
        std::fs::hard_link(&path, &leftover).expect("the leftover");

        let backup = publish_config(&path, "# composed\n", true)
            .expect("a forced publish")
            .expect("it kept the old config");
        assert_eq!(
            std::fs::read_to_string(&backup).expect("the backup"),
            "# the one it replaces\n",
            "the leftover was truncated, so the backup holds the replacement"
        );
        assert_eq!(
            std::fs::read_to_string(&leftover).expect("the leftover"),
            "# the one it replaces\n",
            "the config the leftover names was written through rather than left alone"
        );
        assert_eq!(
            std::fs::read_to_string(&path).expect("the config"),
            "# composed\n"
        );
    }

    #[test]
    fn a_background_read_names_job_control_rather_than_an_io_fault() {
        // TERMIOS(4): a background process that BLOCKS SIGTTIN, which the
        // hidden read does, gets EIO from the read "and no signal is sent",
        // where an unblocked one would have stopped and could be resumed.
        // Passed through raw, `pns setup &` blames an I/O fault for what is
        // job control, and hides the one thing the operator can do about it.
        let eio = std::io::Error::from_raw_os_error(libc::EIO);
        assert!(
            read_failure(&eio, true).contains("bring it to the foreground with fg"),
            "a backgrounded walk was not told why the terminal cannot be read"
        );
        // A HUNG-UP TERMINAL ANSWERS EIO TOO, and that read really did fail
        // for its own reason rather than for job control.
        assert!(
            read_failure(&eio, false).contains("the answers could not be read"),
            "an EIO in the foreground was blamed on job control"
        );
        // AND A BACKGROUND JOB'S OTHER FAILURES KEEP THEIR OWN REASON: a
        // non-UTF-8 paste still has to say that is what happened.
        let other = std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "stream did not contain valid UTF-8",
        );
        assert!(
            read_failure(&other, true).contains("valid UTF-8"),
            "a background job's real read failure was replaced by the job-control line"
        );
    }

    #[test]
    fn a_same_second_backup_collision_names_the_backup_it_could_not_claim() {
        // THE NAME IS CLAIMED WITH `create_new`, so a second forced run inside
        // the same second finds its own stamp already taken; this pre-creates
        // that collision instead of running two forced publishes back to back
        // and hoping they land in the same wall-clock second.
        //
        // THE MOMENT IS NAMED, NOT READ, on both sides: `keep_aside_at`
        // takes the epoch, so this test and the code under it cannot
        // disagree about which second they are in, and exactly one backup
        // name is in play.
        const FIXED_EPOCH: u64 = 1_700_000_000;
        let home = scratch("setup-keep-aside-collision");
        let path = home.join(".config/pns/config.toml");
        std::fs::create_dir_all(path.parent().expect("the directory")).expect("the directory");
        std::fs::write(&path, "# the one it replaces\n").expect("the config");
        let claimed = pns::setup::backup_path(&path, FIXED_EPOCH).expect("the backup name");
        std::fs::write(&claimed, "# an earlier run's own backup\n").expect("the earlier backup");

        let refusal =
            keep_aside_at(&path, FIXED_EPOCH).expect_err("the backup name is already claimed");
        assert!(
            refusal.contains(&claimed.display().to_string()),
            "the refusal does not name the pre-claimed backup: {refusal}"
        );
        assert!(
            refusal.contains("already claimed"),
            "the reason is a raw io::Error instead of naming the same-second collision: {refusal}"
        );
        assert_eq!(
            std::fs::read_to_string(&path).expect("the config"),
            "# the one it replaces\n",
            "the config was moved even though its backup name could not be claimed"
        );
        assert_eq!(
            std::fs::read_to_string(&claimed).expect("the earlier backup"),
            "# an earlier run's own backup\n",
            "an earlier run's own backup was overwritten rather than left alone"
        );
    }

    #[test]
    fn a_claim_that_fails_for_another_reason_is_not_blamed_on_a_same_second_run() {
        // THE CLAIM FAILS, BUT NOT BECAUSE THE NAME IS TAKEN: the config's own
        // directory is missing, so `create_new` cannot open the backup name at
        // all. Only AlreadyExists is the same-second collision; any other
        // failure must carry its own reason rather than blame an earlier run
        // that never happened.
        let home = scratch("setup-keep-aside-other-reason");
        let path = home.join(".config/pns/config.toml");

        let refusal = keep_aside(&path).expect_err("the backup name cannot be claimed");
        assert!(
            refusal.contains("could not be claimed"),
            "the refusal does not say the claim itself failed: {refusal}"
        );
        assert!(
            !refusal.contains("this same second"),
            "a missing directory was blamed on a same-second collision: {refusal}"
        );
    }

    #[test]
    fn a_directory_at_the_config_path_is_named_rather_than_the_backup_it_could_not_replace() {
        // THE RENAME IS WHAT FAILS HERE, not the claim: the backup file is
        // created fine (it is a fresh name), and then a directory cannot be
        // renamed onto it. The refusal is about `path`, the thing that could
        // not be moved, not about `backup`, which was never the problem.
        let home = scratch("setup-keep-aside-directory");
        let path = home.join(".config/pns/config.toml");
        std::fs::create_dir_all(&path).expect("a directory standing where the config belongs");

        let refusal =
            keep_aside(&path).expect_err("a directory cannot be renamed onto a plain file");
        assert!(
            refusal.contains(&path.display().to_string()),
            "the refusal does not name the config path: {refusal}"
        );
        // `backup`'s own display string always carries `path`'s as a prefix
        // (`backup_path` appends `.<stamp>.backup` to the config's name), so
        // checking for the FULL backup string is what actually tells apart a
        // refusal that blames the backup from one that blames the path.
        assert!(
            !refusal.contains(".backup"),
            "the refusal blames the backup file it could not replace path with, \
             rather than the path it could not move: {refusal}"
        );
        assert!(
            path.is_dir(),
            "the directory standing at the config path was moved"
        );
        // THE CLAIMED BACKUP NAME IS RELEASED, not left behind empty: the
        // rename that would have moved the directory onto it never happened,
        // so a `.backup` entry surviving here would be a claim this run made
        // and never used.
        let leftover = leftovers(&path);
        assert!(
            leftover.is_empty(),
            "a backup claim was left behind after the refusal: {leftover:?}"
        );
    }
}
