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
mod command_lights;
mod command_loop;
mod command_nag;
mod command_presence;
mod command_pulse;
mod daemon_child_runtime;
mod daemon_runtime;
mod daemon_spool_runtime;
mod event_flow;
mod event_records;
mod hook_dispatch;
mod hook_observations;
mod hook_payload;
mod invocation;
mod journal_claims;
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
pub(crate) use command_lights::{LIGHTS_QUIET_SAID, ad_hoc_quiet, lights_mode};
pub(crate) use command_loop::{loop_mode, renew_loop_lease};
pub(crate) use command_nag::nag_mode;
pub(crate) use command_presence::{ensure_presence_poll, presence_mode, presence_settings};
pub(crate) use command_pulse::pulse_mode;
pub(crate) use daemon_child_runtime::{Bounded, child_bound, reap, spawn_job};
pub(crate) use daemon_runtime::daemon_run;
pub(crate) use daemon_spool_runtime::drain_spool;
pub(crate) use event_flow::{Attempt, run_event};
pub(crate) use event_records::{
    DECISIONS, MISSED_NOTIFICATIONS, activity_in, record_activity, record_decision, record_missed,
};
pub(crate) use hook_dispatch::hook_mode;
pub(crate) use hook_observations::{
    arm_quota_stale_wait, config_change_detail, model_switch_detail, quota_observation_detail,
    record_policy_settings_change,
};
pub(crate) use hook_payload::{payload_is_whole, read_payload};
pub(crate) use invocation::{USAGE, event_mode, is_producer_argv, second_argument};
pub(crate) use journal_claims::{claim_journal, owner_is_gone};
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

/// The staleness episode this machine was last told about, if any.
fn remembered_staleness() -> Option<String> {
    let episode = std::fs::read_to_string(state_dir().join(STALENESS_MEMORY)).ok()?;
    let episode = episode.trim().to_string();
    (!episode.is_empty()).then_some(episode)
}

/// Remember one staleness episode, or forget one a HOME reading showed
/// resolved. ONLY A HOME READING CALLS THIS: away and unreadable are not
/// resolutions, so they never reach here to erase a live episode.
///
/// FAIL-QUIET in the `start_of_turn` style: an unwritable state directory
/// must never change a verdict, fail the diagnostic, or crash. The cost of a
/// failed write is one repeated warning.
fn remember_staleness(episode: Option<&str>) {
    let memory = state_dir().join(STALENESS_MEMORY);
    let Some(episode) = episode else {
        let _ = std::fs::remove_file(&memory);
        return;
    };
    // The failure is DROPPED here and nowhere else: see the doc comment.
    let _ = publish_state_line(&memory, episode);
}

/// One line, holding the episode the operator has already been warned about,
/// absent when a HOME reading showed no staleness. NO SESSION ID: one config
/// names one device, so there is one staleness state at a time and every
/// reader of it means the same one.
const STALENESS_MEMORY: &str = "home-staleness";

/// Whether the config's hue table resolves to a bridge that could be dialled:
/// the same reading `fire_pulse` takes, taken BEFORE it, so a check can tell a
/// bridge that listed no room from a config that names no bridge at all.
fn hue_resolves(hue_table: Option<&toml::Table>) -> bool {
    hue_table.is_some_and(|settings| {
        hue_settings(settings, std::env::var("HUE_PULSE_ROOMS").ok().as_deref()).is_some()
    })
}

/// What the doctor can say about the lamps, and the ONE place that decides
/// which of its five states this machine is in.
///
/// THE BRIDGE IS DIALLED HERE, and only here, and only for a config that has
/// asked for the lamps AND enabled hue AND named a bridge. It costs the three
/// listings the routing resolves from, whatever the map says: arbitration and
/// the dim window are per lamp, so the joins are needed by every config that
/// routes anything at all.
///
/// BEHIND THE PANIC BOUNDARY every other bridge call gets, for `pulse_outcome`'s
/// reason: a panicking call must cost this section its lines rather than end
/// the report where the operator reads it as complete. A call that panicked
/// resolved no lamp, which is what the unreachable line says.
///
/// THE COST, NAMED: each GET is bounded by `BRIDGE_DEADLINE`, so a bridge that
/// accepts and never answers adds up to thirty seconds to `pns doctor`. That is
/// the same order as the pairing check's own two deadlines and it is paid only
/// by a machine that wrote the table.
fn lights_report(
    lights: Option<&pns::config::Lights>,
    hue_table: Option<&toml::Table>,
    hue_declared: bool,
) -> pns::doctor::LightsReport {
    let Some(lights) = lights else {
        return pns::doctor::LightsReport::Off;
    };
    let Some(settings) = hue_table else {
        // NEVER WRITTEN AND SWITCHED OFF ARE DIFFERENT CONFIGS, and the
        // enabled table is one `None` for both, so the declaration is read
        // separately rather than inferred from its absence.
        return if hue_declared {
            pns::doctor::LightsReport::HueDisabled
        } else {
            pns::doctor::LightsReport::HueMissing
        };
    };
    let Some(hue) = hue_settings(settings, std::env::var("HUE_PULSE_ROOMS").ok().as_deref()) else {
        return pns::doctor::LightsReport::NoBridge;
    };
    let resolved = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        pns::channels::hue::resolve_on_bridge(
            &UreqBridge {
                base: format!("https://{}/clip/v2/resource", hue.bridge),
                key: hue.key,
                deadline: BRIDGE_DEADLINE,
            },
            lights,
        )
    }));
    match resolved {
        Ok(Some(map)) => pns::doctor::LightsReport::Resolved(map),
        Ok(None) | Err(_) => pns::doctor::LightsReport::Unreachable,
    }
}

/// The pulse behind the same boundary every leg gets, so a panicking bridge
/// call costs the census the rest of its lines rather than ending the report
/// where the operator reads it as complete.
fn pulse_outcome(hue_table: Option<toml::Table>) -> pns::doctor::Outcome {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        fire_pulse(hue_table, pns::config::Behaviour::Done)
    })) {
        Ok(rooms) => pns::doctor::Outcome::Signalled(rooms),
        // NO ROOM IS CLAIMED, and no panic text is quoted: the message is
        // written for a developer and may hold anything the pulse was carrying.
        Err(_) => {
            pns::doctor::Outcome::Failed("the pulse PANICKED; no room was signalled".to_string())
        }
    }
}

/// The `home` mode: one reading of the home probe, reported in one line, and
/// the one stale-identifier alert that reading may earn.
///
/// A DIAGNOSTIC FIRST: it always exits 0 and says what it found, including
/// every way it can be unconfigured, because its job is to answer "why did the
/// probe not read" as much as "is the device home". The key itself is never
/// printed, on any path.
///
/// AND THE TRIGGER for the stale-identifier alert, on exactly the condition
/// that prints the warning. This is the only code that reads the sensor and it
/// already holds the derive/decide/remember trio, so one call site keeps ONE
/// memory and ONE decision; a second entrypoint would be a second place for
/// the episode decision to fall out of step. The consequence is deliberate: a
/// hand-run `pns home` no longer consumes an episode silently, it delivers it.
fn home_mode() {
    use pns::home::{HomePresence, SetupFailure, report, setup_report};
    let home_dir = std::env::var("HOME").unwrap_or_default();
    let config = match load_config(&config_path(&home_dir)) {
        Ok(LoadOutcome::Loaded(config)) => config,
        Ok(LoadOutcome::Missing) => {
            println!("{}", setup_report(&SetupFailure::NoConfigFile));
            return;
        }
        Err(error) => {
            println!(
                "{}",
                setup_report(&SetupFailure::ConfigError(error.detail().to_string()))
            );
            return;
        }
    };
    // EVERY CAUSE IS DECIDED IN THE LIBRARY, so each line is pinned by a
    // value-in, value-out test and this stays wiring: a missing table, a
    // disabled one, a `type` nothing answers and a mistyped value each send
    // the operator to a different edit, and one message covering two of them
    // sends half of them to the wrong one.
    let router_table = match pns::home::enabled_router_table(&config) {
        Ok(table) => table,
        Err(failure) => {
            println!("{}", setup_report(&failure));
            return;
        }
    };
    // WHERE THE ALERT GOES, settled at the config read rather than at the
    // post. `hermes_url_for`'s own refusal names `--channel`, a flag nobody
    // typed on this path; this one names the key in the file, and it is said
    // on every run of the diagnostic instead of only on the run that happens
    // to have something to deliver.
    let (alert_route, complaint) = pns::home::stale_alert_channel(router_table);
    if let Some(complaint) = complaint {
        eprintln!("{complaint}");
    }
    let settings = match pns::home::router_settings(router_table) {
        Ok(settings) => settings,
        Err(failure) => {
            println!("{}", setup_report(&failure));
            return;
        }
    };
    // The key stays its own read, so it never joins the settings in a type
    // that could be dumped whole.
    let Some(key) = pns::home::router_api_key(router_table) else {
        println!("{}", setup_report(&SetupFailure::NoApiKey));
        return;
    };
    let router = pns::home::UniFiRouter::new(settings.router_url, key);
    // STILL WIRING: the library decides what is stale, what its episode is
    // called and whether that is news; this reads the memory, prints, and
    // writes the memory back.
    let reading = pns::home::read_home(&router, &settings.device);
    // ONE DERIVATION, ONE DECISION. The episode is spelled once and the news
    // decided once, then the SAME value is what gets printed and what gets
    // remembered: two derivations of one fact, one in the print and one in
    // the write, can only stay in step for as long as neither grows a
    // condition of its own.
    let staleness = pns::home::stale_identifiers(&reading);
    let episode = staleness.as_ref().map(pns::home::episode_id);
    let news = pns::home::is_new_staleness(remembered_staleness().as_deref(), episode.as_deref());
    // ONE VALUE FEEDS BOTH SURFACES. The sentence the terminal prints and the
    // sentence the alert carries come out of this same Option, so there is no
    // second condition that could deliver what was not printed, or print what
    // was not delivered. It is Some only for a HOME reading with a
    // disagreement that is news, which is what keeps away, unreadable and
    // already-told runs silent without a guard of their own.
    let alert = staleness.as_ref().filter(|_| news);
    println!("{}", report(&reading, alert));
    // THE WARNING, DELIVERED. An ordinary event ABOUT the reading, handed to
    // the one event path: presence, surface and the leg plan decide where it
    // lands exactly as they do for a finished agent turn. Nothing narrows it
    // and it is not long-running, so it raises no pulse.
    //
    // DISPATCH BEFORE REMEMBER, AND THE ORDER IS LOAD-BEARING. Tidied into
    // remember-then-dispatch it would silently LOSE an alert: a crash, a
    // wedged channel or a kill between the two leaves the episode recorded
    // and never delivered, and the next run reads it as already told. This
    // way round the same interruption re-alerts instead, and two overlapping
    // hand runs that both read the memory before either writes both alert.
    // Duplicates are the direction to fail in.
    //
    // THE COST, ACCEPTED: the delivery OUTCOME is not consulted before the
    // write either, so a post the gateway rejected consumes the episode just
    // as a delivered one does. Fire-and-forget is this engine's contract for
    // every producer, and the printed line above has already told the one
    // human who typed the command.
    if let Some(staleness) = alert {
        run_event(
            &pns::args::EventArgs {
                agent: "pns".to_string(),
                state: "stale".to_string(),
                detail: pns::home::stale_warning(staleness),
                channel: alert_route,
                ..Default::default()
            },
            &system_probes(),
            &HookPayload::default(),
            Attempt::First,
        );
    }
    // ONLY A HOME READING HAS AN OPINION ABOUT THE IDENTIFIERS. NotHome and
    // Unknown both hand `stale_identifiers` a None, and writing that back
    // would read "the disagreement resolved" out of a trip to the shops or a
    // five-second router timeout: the same invention as reading a failed
    // fetch as NotHome, one layer up. Away and unreadable leave the memory
    // untouched, so the warning stays once per STATE rather than once per
    // homecoming.
    if matches!(reading.presence, HomePresence::Home { .. }) {
        remember_staleness(episode.as_deref());
    }
}

/// The `doctor` mode: one test send through every enabled channel, and one
/// line per REGISTERED plugin about what happened.
///
/// EVERY SUPPRESSION GATE IS BYPASSED, and structurally rather than by a flag.
/// `decide()` is never called, so the presence verdict, the viewed-pane rule
/// and the two phone overrides have nothing to say here; the mute is read in
/// `run_event`, which this is not on; and the pulse goes through `fire_pulse`,
/// the hand-run path `pns pulse` uses, so the lights' quiet window never sees
/// it either. A check that can be suppressed proves nothing about the channel
/// it was checking, and every one of those gates exists to stop a destination
/// receiving.
///
/// THE CENSUS IS THE WHOLE ROSTER, never the selection: a plugin the config
/// left off has to be VISIBLY absent by choice, or the report answers "what is
/// on" when the operator asked "what will reach me".
///
/// EVERY SEND GOES THROUGH THE ENGINE'S OWN WIRING, down to the constructors
/// and `dispatch_legs`, so a doctor cannot report green through a path an
/// event would not use.
fn doctor_mode() -> i32 {
    // ANY EXTRA WORD IS A REFUSAL, before anything is sent or printed. A
    // doctor that quietly ignored an argument is a check the operator believes
    // was narrower or wider than it was.
    if std::env::args_os().nth(2).is_some() {
        eprintln!("{DOCTOR_USAGE}");
        return 2;
    }
    println!("{DOCTOR_OPENING}");

    let home = std::env::var("HOME").unwrap_or_default();
    let loaded = load_config(&config_path(&home));
    // The same readings `run_event` takes off the same config, before
    // selection consumes it.
    let (
        hue_table,
        mobile,
        hermes_key,
        replay_card,
        focus_silence,
        daemon_enabled,
        nag_after_secs,
        lights,
        hue_declared,
    ) = match &loaded {
        Ok(LoadOutcome::Loaded(config)) => (
            enabled_hue_table(config),
            read_mobile(config),
            plugin_settings(config, "hermes").and_then(hermes_secret),
            config.recap.replay_card,
            config.focus_silence.clone(),
            config.daemon_enabled,
            config.nag_after_secs,
            config.lights.clone(),
            // WHETHER THE TABLE WAS WRITTEN AT ALL, which
            // `enabled_hue_table` cannot say: it answers `None` both for a
            // table nobody wrote and for one whose switch is off, and the
            // lamps' report tells those two apart.
            config.plugins.contains_key("hue"),
        ),
        // THE SWITCH FALLS BACK ON, which is the fallback `run_event` takes
        // for the same reading. The two must agree or the doctor describes a
        // delivery the event would not make, and the Focus list falls back
        // EMPTY here for the same reason it does there.
        // AND THE NAG FALLS BACK OFF, which is the fallback `nag_after_secs`
        // takes for the same reading: the two must agree or the doctor
        // describes a schedule the fire would not keep.
        _ => (
            None,
            Mobile::default(),
            None,
            true,
            Vec::new(),
            true,
            NAG_OFF,
            None,
            false,
        ),
    };
    // THE SWITCHED-OFF TABLES THE EVENT PATH SAYS NOTHING ABOUT, said here
    // and only here: see `disabled_backend_warning`.
    if let Ok(LoadOutcome::Loaded(config)) = &loaded {
        for warning in disabled_backend_warnings(config) {
            eprintln!("{warning}");
        }
    }
    // THE ROOM SENSOR'S OWN SETTINGS, read here because the census below has
    // one line to print about them. A refusal is LOUD and leaves the reading
    // absent rather than half-honoured.
    let presence_settings = match &loaded {
        Ok(LoadOutcome::Loaded(config)) => match pns::config::parse_presence(config) {
            Ok(settings) => settings,
            Err(error) => {
                eprintln!(
                    "pns: config error ({}); the room sensor is unread",
                    error.detail()
                );
                None
            }
        },
        _ => None,
    };
    let registry = roster();
    // WHAT LOADING FOUND, taken BEFORE `select_plugins` consumes it: the
    // census reports a plugin the selection left out, and which sentence is
    // true of that depends entirely on whether there was a config to read.
    let config_state = match &loaded {
        Ok(LoadOutcome::Loaded(_)) => pns::doctor::ConfigState::Read,
        Ok(LoadOutcome::Missing) => pns::doctor::ConfigState::Absent,
        Err(_) => pns::doctor::ConfigState::Unreadable,
    };
    // THE CONFIG FALLBACK IS INHERITED ON PURPOSE. `select_plugins` is what an
    // event would run and warn about, and the doctor's job is to say what an
    // event would do, not what a tidier engine would do.
    let (selection, warning) = select_plugins(&registry, loaded);
    if let Some(warning) = warning {
        eprintln!("{warning}");
    }
    let checks = pns::doctor::checks(&registry.all(), &selection, config_state);

    let event = pns::args::EventArgs {
        agent: "pns".to_string(),
        state: "doctor".to_string(),
        detail: DOCTOR_DETAIL.to_string(),
        ..Default::default()
    };
    let legs: Vec<pns::routing::Leg> = checks
        .iter()
        .filter(|check| check.kind == pns::doctor::CheckKind::Send)
        .map(|check| pns::routing::Leg {
            name: check.plugin,
            // The operator is standing here waiting for the answer, which is
            // what the reporting mode means and which deadline hermes posts
            // under. It decides nothing about who hears the report: the doctor
            // prints every outcome itself.
            mode: pns::routing::ReportMode::ReportOutcome,
            // NOT A DECORATION, because no plan chose these: the doctor
            // bypasses every gate and sends to whatever is enabled. The flag
            // says a leg is there BECAUSE the operator was to be shown
            // something, and the honest answer here is no.
            decorative: false,
        })
        .collect();
    // NO PANE: its only consumer is the banner's click target, and whether a
    // click focuses the right pane cannot be verified without a human clicking
    // it, so carrying one would add the scrub rule to a second call site to
    // test nothing this can observe.
    let delivered = dispatch_legs(&legs, false, &event, &home, &mobile, hermes_key);

    let outcomes: Vec<pns::doctor::Outcome> = checks
        .iter()
        .map(|check| match check.kind {
            pns::doctor::CheckKind::Skipped(reason) => pns::doctor::Outcome::Skipped(reason),
            // NOTHING IS DIALLED FOR SETTINGS THAT RESOLVE TO NO BRIDGE.
            // `fire_pulse` answers zero rooms for that config exactly as it
            // does for a bridge that listed none, and the zero-rooms line
            // blames the listing or the room names: both wrong here, and both
            // send the operator hunting through a bridge nothing contacted.
            pns::doctor::CheckKind::Pulse if !hue_resolves(hue_table.as_ref()) => {
                pns::doctor::Outcome::Failed(NO_HUE_BRIDGE_LINE.to_string())
            }
            pns::doctor::CheckKind::Pulse => pulse_outcome(hue_table.clone()),
            // A READING, NEVER A SEND: nothing is dispatched to a sensor, and
            // what an operator cannot see any other way is what it says now.
            pns::doctor::CheckKind::Presence => pns::doctor::Outcome::Presence(
                presence_status(presence_settings.as_ref()),
                last_narrowing(&state_dir()),
            ),
            // BY NAME, never by position. The legs above are these checks in
            // this order and `dispatch_legs` answers one outcome per leg, so
            // the two agree today; a positional pairing that ever stopped
            // agreeing would print one channel's verdict under another's
            // label, which is a silent misreport rather than a visible one.
            // The absent case cannot happen and still reports a problem rather
            // than claiming a send, which is the direction to be wrong in.
            pns::doctor::CheckKind::Send => {
                match delivered.iter().find(|(leg, _)| leg.name == check.plugin) {
                    Some((_, Delivery::Delivered(said))) => {
                        pns::doctor::Outcome::Sent(said.clone())
                    }
                    Some((_, Delivery::Failed(said) | Delivery::Unlaunched(said))) => {
                        pns::doctor::Outcome::Failed(said.clone())
                    }
                    // Silent BY DESIGN, which is an executable channel that
                    // RAN: it was handed the event and has no second surface
                    // to answer on.
                    Some((_, Delivery::Silent)) => pns::doctor::Outcome::SentUnreported,
                    None => {
                        pns::doctor::Outcome::Failed("the leg was never dispatched".to_string())
                    }
                }
            }
        })
        .collect();

    for (check, outcome) in checks.iter().zip(&outcomes) {
        println!("{}", pns::doctor::line(check, outcome));
    }
    println!("{}", pns::doctor::summary(&outcomes));
    // BETWEEN THE SUMMARY AND THE DECISION SECTION, which is health beside
    // health and history last: this check can move the exit code and the
    // decision log explicitly cannot, so the other order would put a gradeable
    // line below an ungradeable one.
    let pairing = read_pairing();
    for line in pns::doctor::pairing_lines(&pairing) {
        println!("{line}");
    }
    // GATE STATE ABOVE THE HISTORY THE GATE EXPLAINS, and below the pairing
    // check, which is health. It must NOT move the exit code, for the reason
    // the decision section does not: a Focus being on is not a fault.
    println!("{}", focus_line(&home, &focus_silence));
    // BESIDE THE FOCUS LINE, which is the other line that reports state without
    // grading it. It must NOT move the exit code in any state, including the
    // dead one: a daemon that is down costs ambient features, and this exit
    // code is what an operator's automation reads as "notifications are
    // broken".
    println!("{}", daemon_line(daemon_enabled));
    // IMMEDIATELY BELOW THE DAEMON'S OWN LINE, and that placement is the whole
    // mitigation for the one thing this line does not say: a nag with a dead
    // daemon never fires, and the line above already reports the daemon from its
    // heartbeat. Two lines deriving one fact is how they drift apart, so these
    // two read as one paragraph instead.
    println!("{}", pns::doctor::nag_line(nag_after_secs));
    // AND THE LAMPS BELOW THE GATE, for the same reason: a dark lamp is not a
    // broken notifier, so this section reports and never grades. It is the last
    // thing that touches the network, so a bridge that hangs cannot delay a
    // line above it.
    for line in pns::doctor::lights_lines(&lights_report(
        lights.as_deref(),
        hue_table.as_ref(),
        hue_declared,
    )) {
        println!("{line}");
    }
    // APPENDED AFTER THE SUMMARY, which is what lets it be added at all: the
    // census plus its summary is one complete thought whose line order the
    // suite already pins, and nothing below can disturb it.
    for line in decision_section() {
        println!("{line}");
    }
    // HISTORY BELOW HISTORY, and last for the reason the decision section is
    // second to last: an unreplayed journal is not a failure, so it sits under
    // the one section that already cannot move the exit code.
    println!("{}", missed_line(replay_card));
    // THE DECISION SECTION DOES NOT MOVE THE EXIT CODE. It reports HISTORY,
    // not health: an empty log on a fresh machine is not a failure, and
    // neither is one nothing could read. The pairing IS health and does move
    // it, which is why it is an argument rather than a second code combined
    // here: one decision point, decided in one place.
    pns::doctor::exit_code(&outcomes, &pairing)
}

/// What moshi-hook says about this host's pairing, in TWO BOUNDED SPAWNS of
/// one subcommand.
///
/// The split is a correctness argument rather than a style one. `status
/// --json` is local-only, measured at 77ms with the base URL pointed at an
/// unroutable host, and it carries the pairing fact pns grades. Plain `status`
/// is the only shape carrying a server verdict and is the only thing the
/// doctor puts on the network for its own sake. One plain-only call would put
/// the local fact behind the network, so an outage would read as "pairing
/// could not be checked" on a machine that could have answered.
///
/// `probe` IS NEVER CALLED. Measured on 0.3.3, it answers `running: true` and
/// `gateway: true` against a HOME holding no pairing at all while its hostId
/// disappears, so its daemon-side provenance cannot be stated honestly.
///
/// A FORWARD RISK, named rather than coded around: every pairing state exits 0
/// today, so a future moshi that exited non-zero when unpaired would come back
/// as no answer and be reported as "could not check" while the approval path
/// is really dead. A future moshi that renamed or dropped the `server:` line
/// degrades the other way, silently and safely.
///
/// THE WORST CASE IS THE TWO DEADLINES ADDED, not the larger of them: the legs
/// run one after the other, so a moshi-hook wedged on both puts 5s + 8s on a
/// hand-typed command, measured at 13.07 seconds. Ten seconds is not the
/// bound and nobody should treat it as one.
fn read_pairing() -> pns::doctor::PairingReport {
    let binary = moshi_hook_bin();
    let mut json = Command::new(&binary);
    json.args(["status", "--json"]);
    // WELL PAST THE CHECK'S OWN CAP on both legs, so an answer over that cap
    // still ARRIVES over it: read to the cap exactly and a truncated answer
    // would pass the refusal that exists to catch it.
    let json = run_bounded(json, None, moshi_json_deadline(), PAIRING_READ_MAX);
    let mut plain = Command::new(&binary);
    plain.arg("status");
    let plain = run_bounded(plain, None, moshi_status_deadline(), PAIRING_READ_MAX);
    pns::doctor::pairing_report(json.as_deref(), plain.as_deref())
}

/// How much of moshi's answer is read off the wire, which is NOT the same
/// number as how much of it the check will look at.
///
/// TWICE WHAT `doctor::pairing_report` READS, and the doubling is the whole
/// point of the constant. The reader refuses anything past its own ceiling
/// (`system::run_bounded`), and the check refuses anything past
/// `doctor::ANSWER_MAX`, and those two refusals say DIFFERENT things: over the
/// reader's ceiling nothing usable arrived at all, while over the check's cap
/// moshi-hook ran and said something pns declined to read. Read to the check's
/// cap exactly and the second sentence would be unreachable, so the room
/// between them is what keeps it a state an operator can actually be told
/// about. It is still a bound: a child streaming without end is stopped here.
///
/// ACCEPTED LIMIT, PAST THIS CEILING: a moshi-hook that answers with more than
/// two megabytes is reported as a daemon that DID NOT ANSWER, because that is
/// the only thing the reader can say about an answer it refused to read. A
/// wedged daemon streaming prose is then diagnosed as a dead one, which sends
/// the operator to `brew services restart` rather than to the output. The
/// trade is deliberate: the alternative is reading without a ceiling to be
/// able to describe what came back, and the ceiling is the point.
const PAIRING_READ_MAX: u64 = 2 * pns::doctor::ANSWER_MAX as u64;

/// How long `moshi-hook status --json` may take.
///
/// GENEROUS AGAINST A MEASURED 77ms, and pinned here rather than inherited
/// from the probe runner's shared window: this leg reaches no network today,
/// and "today" is exactly why the bound has to be this function's own to state
/// and a test's own to move.
fn moshi_json_deadline() -> Duration {
    env_deadline("PNS_MOSHI_JSON_DEADLINE_MS").unwrap_or(MOSHI_JSON_DEADLINE)
}

const MOSHI_JSON_DEADLINE: Duration = Duration::from_secs(5);

/// How long plain `moshi-hook status` may take.
///
/// IT MUST EXCEED MOSHI'S OWN internal timeout, measured at about 5.1 seconds
/// against an unroutable base URL. Killing it mid-wait would throw away the
/// very `unavailable (...)` sentence that explains the delay, which is the one
/// thing this call is for.
fn moshi_status_deadline() -> Duration {
    env_deadline("PNS_MOSHI_STATUS_DEADLINE_MS").unwrap_or(MOSHI_STATUS_DEADLINE)
}

const MOSHI_STATUS_DEADLINE: Duration = Duration::from_secs(8);

/// The decision ring, read back and rendered.
///
/// READ AND NEVER APPENDED. A doctor that recorded would push the decision the
/// operator came to read out of the ring by the act of going to look at it.
fn decision_section() -> Vec<String> {
    let now = now_secs();
    match pns::system::readable_state_file(&state_dir().join(DECISIONS), RING_READ_MAX) {
        Ok(contents) => pns::decision_log::section(Some(&contents), now),
        // ABSENT IS ITS OWN STATE, and the one the section has an honest line
        // for. Anything else is a directory or a permission problem, which is
        // a different thing to say.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            pns::decision_log::section(None, now)
        }
        Err(error) => vec![format!("{DECISIONS_UNREADABLE} ({}).", error.kind())],
    }
}

/// A ring that is there and cannot be read. Said HERE rather than in the log
/// module, for the reason `NO_HUE_BRIDGE_LINE` is: the sentence needs
/// something only the reader of the file knows.
const DECISIONS_UNREADABLE: &str = "pns doctor: the decision log could not be read";

/// The missed-notification journal, COUNTED and never rendered.
///
/// READ AND NEVER APPENDED, for the reason the decision section is: a doctor
/// that journaled would file a miss for the act of going to look for one, and
/// its own test send is the last event anything should ever replay.
///
/// NOTHING HERE PARSES AN ENTRY. The contents go straight to `waiting_line`,
/// which counts lines and has no parse at all, so the operator's own text has
/// no path from this file to a terminal.
///
/// `replay_card` REACHES THE SENTENCE because the sentence makes a promise.
/// With the card switched off nothing will ever deliver what is counted here,
/// and a doctor that still named "the next event" would be telling the
/// operator a lie their own setting makes permanent.
fn missed_line(replay_card: bool) -> String {
    match pns::system::readable_state_file(&state_dir().join(MISSED_NOTIFICATIONS), RING_READ_MAX) {
        Ok(contents) => pns::missed_notifications::waiting_line(Some(&contents), replay_card),
        // ABSENT IS ITS OWN STATE, and the one the line has an honest sentence
        // for. Anything else is a directory or a permission problem, which is
        // a different thing to say.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            pns::missed_notifications::waiting_line(None, replay_card)
        }
        Err(error) => format!("{MISSED_UNREADABLE} ({}).", error.kind()),
    }
}

/// A journal that is there and cannot be read. Said HERE rather than in the
/// module, for the reason `DECISIONS_UNREADABLE` is.
const MISSED_UNREADABLE: &str = "pns doctor: the missed-notification journal could not be read";

/// What Focus is doing to this machine right now, in one sentence.
///
/// THE UNREADABLE STATE IS WHAT EARNS THE LINE. If the store is ever gated
/// behind Full Disk Access, moves, or changes schema, this feature dies OPEN
/// and SILENT: pns simply stops respecting Focus, and nothing else anywhere
/// would ever say so.
///
/// THE ACCEPTED LIMIT, stated rather than designed around: the parser is
/// TOTAL, so bytes that are not JSON at all, and a schema change that leaves
/// the file valid JSON, both read as "no Focus" rather than as an error. Only
/// a failed READ of the file itself reaches the last two sentences, and a
/// store that had stopped being readable in any useful sense would still be
/// reported as quiet. Telling those apart needs a positive assertion about a
/// shape Apple promises nothing about.
///
/// FIVE SENTENCES, because ABSENT AND UNREADABLE ARE DIFFERENT THINGS TO SAY,
/// which is the rule `decision_section` and `missed_line` already follow one
/// screen up. A machine that has never asserted a Focus has no store, and
/// telling that operator their database could not be read sends them after a
/// Full Disk Access grant that was never the problem.
fn focus_line(home: &str, silence: &[String]) -> String {
    if silence.is_empty() {
        return "pns doctor: focus awareness is off (no [focus] table names a mode to silence)"
            .to_string();
    }
    match focus_now(home, silence) {
        Ok(reading) => {
            let state = if reading.silenced {
                "pns doctor: a macOS Focus you named is ON, so banners, cards and pulses \
                 are suppressed"
            } else {
                "pns doctor: no macOS Focus you named is active"
            };
            // A CATALOG NOBODY CAN READ RESOLVES NO NAMES, so a config written
            // the way the template shows it silences nothing while this line
            // otherwise reports perfect health. WHICH entries are names is not
            // decidable without the very file that failed, so the clause is
            // said whenever the catalog failed and the feature is on.
            match reading.catalog {
                None => state.to_string(),
                Some(kind) => format!(
                    "{state}; the mode catalog could not be read ({kind}), so no Focus NAME \
                     can match and only a raw modeIdentifier still would"
                ),
            }
        }
        // ABSENT IS ITS OWN STATE, and the one this machine is in until macOS
        // first writes the store. Anything else is a permission problem, a
        // path holding something that is not a file, or a store past the read
        // ceiling, which is a different thing to say.
        //
        // IT REPORTS WHAT WAS OBSERVED, "no database was found", rather than
        // asserting there is none. Whether a Full Disk Access refusal can
        // arrive as not-found rather than as a permission error is not
        // provable on a machine that holds the grant, so the sentence is
        // written to stay true either way.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            "pns doctor: no Focus database was found on this machine, so no Focus is being \
             respected"
                .to_string()
        }
        Err(error) => format!("{FOCUS_UNREADABLE} ({}).", error.kind()),
    }
}

/// Whether the clock is running, said in one line that grades nothing.
///
/// TWO READS THAT COST NOTHING: the heartbeat file, and a count of the spool.
/// IT DOES NOT SIGNAL THE PID, because a pid can be reused and the age of a
/// file the daemon rewrites every second answers the same question honestly.
/// `enabled` COMES FROM THE ONE CONFIG READ the doctor already took, never a
/// second one: a report assembled from two reads of one file can describe a
/// switch the run itself never saw. Its broken-config fallback is ON, the same
/// one `daemon_run` takes, so the report and the service cannot disagree.
fn daemon_line(enabled: bool) -> String {
    let state = state_dir();
    let path = pns::daemon::heartbeat_path(&state);
    // A NON-REGULAR FILE IS NOT A BEAT AND IS NEVER OPENED, the same refusal
    // the spool takes and for a worse reason: `open` on a FIFO blocks until a
    // writer arrives, so a doctor that read whatever it found there would hang
    // instead of printing any of its four states, with the pairing check and
    // the exit code never reached.
    let beat = matches!(std::fs::symlink_metadata(&path), Ok(found) if found.is_file())
        .then(|| std::fs::read_to_string(&path).ok())
        .flatten()
        .and_then(|line| pns::daemon::parse_heartbeat(&line));
    pns::doctor::daemon_line(enabled, beat, now_secs(), pns::daemon::job_count(&state))
}

/// A Focus store that is there and cannot be read. Said HERE rather than in
/// the module, for the reason `DECISIONS_UNREADABLE` is, and carrying the KIND
/// for the reason its two neighbours do: gated, oversized and not-a-file are
/// three different investigations.
const FOCUS_UNREADABLE: &str =
    "pns doctor: the Focus database could not be read, so Focus is being ignored";

/// What a doctor typed wrong is told. ONE WORD AND NO FLAGS: a namespace built
/// for callers that do not exist makes the common case longer to type, and the
/// report absorbs a new section without a new spelling.
const DOCTOR_USAGE: &str = "pns: usage: pns doctor";

/// The contract, STATED rather than measured. Whether a gate is currently in
/// effect is the decision log's question, and reporting live gate state here
/// would be that feature built twice, in two places, from two readings.
const DOCTOR_OPENING: &str = "pns doctor: sending one test to every enabled channel. \
     Every suppression gate is bypassed (the operator mute, a macOS Focus you named, \
     the presence gate, the viewed-pane rule, the lights' quiet hours), because a check \
     that can be suppressed proves nothing.";

/// The line for lights that were selected and never set up. It names the
/// settings to write, the way moshi's and hermes's do, because "no rooms"
/// without an address sends the operator to a bridge nothing dialled.
const NO_HUE_BRIDGE_LINE: &str = "pulse SKIPPED -- no hue bridge and key in the config \
     ([plugins.hue] bridge, key); nothing was signalled";

/// The payload's detail, so whoever the card wakes knows at once that nothing
/// is wrong and nothing needs doing.
const DOCTOR_DETAIL: &str = "test send from pns doctor; nothing is wrong and nothing needs doing";

/// The `recap` mode: one window of activity, rendered and posted, in a process
/// nobody is waiting on.
///
/// IT TAKES NO DECISION, which is what makes it a mode. The decision was taken
/// by the event that spawned it, and re-deciding here would be the second
/// reading of one moment `GateInputs` exists to forbid.
///
/// IT REACHES ONE DESTINATION, the durable route, and never the phone or the
/// banner. The phone layer was already delivered by the card that pointed here.
///
/// EXIT 2 FOR A MISTYPED INVOCATION, in `quiet_mode`'s style rather than the
/// hook path's always-zero: this is hand-runnable, and a subcommand that
/// swallows a typo is a recap the operator believes was posted. The spawner
/// never reads the code.
fn recap_mode() -> i32 {
    let arguments: Vec<String> = std::env::args_os()
        .skip(2)
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect();
    let Some((since, until)) = recap_bounds(&arguments) else {
        eprintln!("{RECAP_USAGE}");
        return 2;
    };
    let home = std::env::var("HOME").unwrap_or_default();
    // FAIL CLOSED ON THE ROUTE AND ON THE SUMMARIZER, AND OPEN ON THE POST,
    // which is `pulse_mode`'s split: a config nobody can read named no route
    // and no command, so the recap goes to the default route, plainly, rather
    // than to a route the operator never asked for or through a program they
    // never named.
    let (hermes_key, recap) = match load_config(&config_path(&home)) {
        Ok(LoadOutcome::Loaded(config)) => (
            plugin_settings(&config, "hermes").and_then(hermes_secret),
            config.recap,
        ),
        _ => (
            None,
            pns::config::Recap {
                digest_as_thread: false,
                ..Default::default()
            },
        ),
    };
    let entries = activity_in(since, until);
    // THE TWO EXTERNAL SOURCES ARE READ ONLY WHEN A KEY NAMES THEM, and both
    // are read HERE, in the process nobody is waiting on. A repository listing
    // is a network call somebody else's machine answers and a glob is a
    // directory read; neither belongs anywhere near the card, and neither is
    // allowed to cost the rest of the recap anything when it does not come
    // back.
    let fetched_merges =
        (!recap.repos.is_empty()).then(|| merged_pull_requests(&recap.repos, since, until));
    let fetched_notes = recap
        .review_notes
        .as_deref()
        .map(|pattern| notes_matching(pattern, &home, since, until));
    // ONE EPISODE, ONE BUDGET. The locked "the LLM runs once at the return
    // moment" is a moment rather than a call: this recap asks up to three
    // questions (the night, the merges, the notes) and `summarizer_deadline_secs`
    // is what the WHOLE episode may spend, so each call is bounded by what is
    // left of it. Per-call deadlines meant a 240-second key could hold two
    // processes for twelve minutes while the card had already said the recap
    // was in #pns, and a laptop that sleeps inside that window loses the recap
    // entirely. Adjudicated 2026-08-29.
    let episode = std::time::Instant::now() + Duration::from_secs(recap.summarizer_deadline_secs);
    // THE ANSWER IS TAKEN BEFORE THE BODY IS COMPOSED and nothing else waits on
    // it: this process was started so that a model could be slow somewhere
    // nobody is standing.
    // AND NOT OVER AN EMPTY WINDOW. A night with nothing in it has nothing to
    // select from, and the model would be handed "nothing was recorded in this
    // window" under an instruction to rewrite it as a timeline. That is a
    // process spawned to summarize nothing and an invitation to invent, on the
    // one path an operator reaches by hand.
    let answered = recap
        .summarizer
        .as_deref()
        .filter(|_| !entries.is_empty())
        .map(|argv| {
            summarize(
                argv,
                left_of(episode),
                &pns::recap::prompt(&entries, &|at| wall_clock(at)),
            )
        });
    let timeline = match &answered {
        None => pns::recap::Timeline::Mechanical,
        Some(None) => pns::recap::Timeline::Unanswered,
        Some(Some(lines)) => pns::recap::Timeline::Summarized(lines),
    };
    // ONE SUMMARIZER CALL PER SECTION, and each falls back on its own. They are
    // three different questions over three different sets of text, so one call
    // answering all three would need the backend to keep them apart in its
    // answer, and a section would then be lost to a separator a model got wrong
    // rather than to anything pns could see. THEY SHARE ONE DEADLINE, above: a
    // call reached with the episode's budget already spent is never started.
    let merge_lines = read_sources(&fetched_merges)
        .and_then(|sources| summarized(&recap, episode, sources, pns::recap::merge_prompt));
    let note_lines = read_sources(&fetched_notes)
        .and_then(|sources| summarized(&recap, episode, sources, pns::recap::note_prompt));
    let externals = pns::recap::Externals {
        merges: pns::recap::External {
            found: found(&fetched_merges),
            answered: merge_lines.as_deref(),
            truncated: truncated(&fetched_merges),
        },
        notes: pns::recap::External {
            found: found(&fetched_notes),
            answered: note_lines.as_deref(),
            truncated: truncated(&fetched_notes),
        },
    };
    let body = pns::recap::body(
        &entries,
        &wall_clock(Some(since)),
        &wall_clock(Some(until)),
        &|at| wall_clock(at),
        timeline,
        &externals,
    );
    post_recap(&body, recap.digest_as_thread, &home, hermes_key)
}

/// What one external source held, and whether a cap stopped the read short of
/// everything there was.
///
/// TRUNCATION TRAVELS WITH THE SOURCES rather than being recomputed from their
/// length, because the two caps are different facts: a listing that came back at
/// exactly `GH_LIMIT` may have more behind it, and a glob matching more files
/// than `MAX_NOTES` certainly does. Only the fetch knows which, and the message
/// says "at least" on either.
struct Fetched {
    sources: Vec<pns::recap::Sourced>,
    truncated: bool,
}

/// One external source's three states, said in the type the body reads.
///
/// THE OUTER `Option` IS THE KEY AND THE INNER ONE IS THE READ, which is what
/// keeps "nobody configured this" and "this would not answer" apart all the way
/// from the config to the message. An empty `Vec` is neither: it is a source
/// that answered with nothing.
fn found(fetched: &Option<Option<Fetched>>) -> pns::recap::Found<'_> {
    match fetched {
        None => pns::recap::Found::Unconfigured,
        Some(None) => pns::recap::Found::Unavailable,
        Some(Some(fetched)) => pns::recap::Found::Read(&fetched.sources),
    }
}

/// Whether what `found` holds is a floor. A source nobody configured and one
/// that would not answer are neither: there is no count to qualify.
fn truncated(fetched: &Option<Option<Fetched>>) -> bool {
    matches!(fetched, Some(Some(fetched)) if fetched.truncated)
}

/// What a source actually held, for the two callers that only have something to
/// do when it held anything.
fn read_sources(fetched: &Option<Option<Fetched>>) -> Option<&[pns::recap::Sourced]> {
    Some(fetched.as_ref()?.as_ref()?.sources.as_slice()).filter(|sources| !sources.is_empty())
}

/// What the summarizer said about one external section, or None for every way
/// of not having an answer.
///
/// NOT OVER AN EMPTY SOURCE, which is `recap_mode`'s own rule about an empty
/// window applied a second time: a model handed nothing to select from is a
/// process spawned to summarize nothing and an invitation to invent.
fn summarized(
    recap: &pns::config::Recap,
    episode: std::time::Instant,
    sources: &[pns::recap::Sourced],
    prompt: fn(&[pns::recap::Sourced]) -> String,
) -> Option<Vec<String>> {
    summarize(
        recap.summarizer.as_deref()?,
        left_of(episode),
        &prompt(sources),
    )
}

/// What is left of the episode's one budget. Zero once it is spent, which
/// `summarize` reads as a call not worth starting.
fn left_of(episode: std::time::Instant) -> Duration {
    episode.saturating_duration_since(std::time::Instant::now())
}

/// The pull requests merged into the named repositories inside the window, or
/// None when the listing could not be had.
///
/// `gh` CARRIES ITS OWN AUTH AND THIS NEVER TOUCHES IT. No token is read, no
/// credential is passed and no network call is made by pns itself: the one
/// spawn is a LIST, and the whole feature is read-only by construction, which
/// is what bounds a pull request body being somebody else's text.
///
/// RESOLVED THROUGH PATH, like `herdr` and unlike the system binaries: it is
/// installed wherever this machine's package manager put it, and a context
/// whose PATH does not carry it reads as unavailable, which costs this section
/// and nothing else.
///
/// BOUNDED THREE WAYS, because every one of them is a way for a remote answer
/// to become this machine's problem: the window is stated in the search so the
/// service does the selecting, `--limit` caps how many come back, and the read
/// is capped in time and in bytes by the seam. A truncated listing is not JSON,
/// so the cap fails CLOSED into "unavailable" rather than into a half-read
/// section.
///
/// ANY REPOSITORY FAILING FAILS THE SECTION, deliberately. A partial list under
/// a count is a count that lies, and this section's remainder line is counted
/// against what was read.
///
/// THE WINDOW IS THE RECAP'S OWN, SHIFTED ONE SECOND. GitHub's range syntax is
/// inclusive at both ends and `activity_in`'s window is `(since, until]`, so a
/// pull request merged in the marker's own second would be fetched while every
/// event in that second is excluded. Starting the search a second later is the
/// same bracket the rest of the recap uses. ACCEPTED LIMIT: the search's
/// granularity is one second, so this is exact rather than approximate only
/// because both bounds are whole seconds to begin with.
///
/// ACCEPTED LIMIT: THE SEARCH INDEX TRAILS THE MERGE, by seconds to minutes. A
/// pull request merged shortly before the return moment can be absent from this
/// listing with no signal, and the next window opens after it, so it is never
/// reported at all. Stating the window server-side is still right (the
/// alternative is fetching everything and selecting here), and the tail pointer
/// to the repository is what closes it.
///
/// ACCEPTED LIMIT: the receipt is the pull request NUMBER, so two repositories
/// merging the same number inside one window produce two lines that cite it.
/// Both are real merges the operator can follow; the alternative is a receipt
/// carrying a repository name, which costs every line its width for a case one
/// configured repository never reaches.
fn merged_pull_requests(repos: &[String], since: u64, until: u64) -> Option<Fetched> {
    let window = format!(
        "merged:{}..{}",
        pns::system::utc_timestamp(since.checked_add(1)?)?,
        pns::system::utc_timestamp(until)?
    );
    let mut merged = Vec::new();
    let mut truncated = false;
    for repo in repos {
        let mut command = Command::new(GH);
        command.args([
            "pr",
            "list",
            "--repo",
            repo,
            "--state",
            "merged",
            "--search",
            &window,
            "--json",
            "number,title,body",
            "--limit",
            &GH_LIMIT.to_string(),
        ]);
        let listing = run_bounded(command, None, GH_DEADLINE, GH_READ_MAX)?;
        let entries = serde_json::from_str::<Vec<serde_json::Value>>(&listing).ok()?;
        // A LISTING THAT CAME BACK AT ITS OWN LIMIT MAY HAVE MORE BEHIND IT,
        // and nothing here can tell a repository with exactly fifty merges from
        // one with five hundred. The count the section prints then says "at
        // least", which is the honest reading of a cap.
        truncated |= entries.len() >= GH_LIMIT;
        for entry in entries {
            // THE NUMBER IS REQUIRED AND ITS ABSENCE FAILS THE WHOLE READ: it
            // is the receipt, so an entry without one is a line nobody could
            // follow, and an answer shaped like that is not the listing that
            // was asked for.
            let number = entry.get("number").and_then(serde_json::Value::as_u64)?;
            merged.push(pns::recap::merged(
                number,
                field(&entry, "title"),
                field(&entry, "body"),
            ));
        }
    }
    Some(Fetched {
        sources: merged,
        truncated,
    })
}

/// One string off a listing entry, or empty. A short entry degrades to a
/// thinner line, which is `missed_notifications::entries`'s own rule.
fn field<'entry>(entry: &'entry serde_json::Value, key: &str) -> &'entry str {
    entry
        .get(key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
}

/// The review notes the glob names whose own clock falls inside the window, or
/// None when the directory the operator named could not be read at all.
///
/// THE GLOB IS THE WHOLE PERMISSION and this is where that is spent: one
/// directory, named in full by the operator, listed once. Nothing recurses,
/// nothing follows a name the pattern did not match, and the config layer has
/// already refused a pattern whose DIRECTORY carries a `*`, so the set of
/// directories pns opens is a statement rather than a search.
///
/// THE WINDOW IS `activity_in`'S OWN PREDICATE, so a note is in this recap for
/// the same reason an event is: it happened after the operator was last here.
/// A note they had already read when they left is not news.
///
/// EVERY READ IS BOUNDED and every file is a file: a directory or a device
/// entry matching the pattern is skipped rather than opened, and what is read
/// stops at a ceiling, because this is an ordinary directory other tools also
/// write into.
///
/// NEWEST FIRST, WHICH IS WHAT THE CAP THEN CUTS. Sorting by name and taking
/// the first `MAX_NOTES` kept whatever sorted earliest, so `checklist-a*.md`
/// outranked the note written an hour ago, which is the opposite of what a
/// section about the night wants. The name breaks a tie, so one window still
/// renders the same way twice.
///
/// ACCEPTED LIMIT: past `MAX_NOTES` the count is a FLOOR rather than a total,
/// which is the honesty `header` states about a pruned ring. The MESSAGE says
/// so now: `Fetched::truncated` is what turns the section's remainder into "at
/// least", so a glob matching forty notes cannot print a count that reads as a
/// total.
fn notes_matching(pattern: &str, home: &str, since: u64, until: u64) -> Option<Fetched> {
    let expanded = match pattern.strip_prefix("~/") {
        Some(rest) => Path::new(home).join(rest),
        None => std::path::PathBuf::from(pattern),
    };
    let name = expanded.file_name()?.to_str()?.to_string();
    let mut matched: Vec<(Duration, std::path::PathBuf)> = std::fs::read_dir(expanded.parent()?)
        .ok()?
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(std::ffi::OsStr::to_str)
                .is_some_and(|found| matches_glob(found, &name))
        })
        .filter_map(|path| Some((modified_at(&std::fs::metadata(&path).ok()?)?, path)))
        .filter(|(at, _)| within(*at, since, until))
        .collect();
    matched.sort_by(|(left, left_path), (right, right_path)| {
        right.cmp(left).then_with(|| left_path.cmp(right_path))
    });
    Some(Fetched {
        truncated: matched.len() > MAX_NOTES,
        sources: matched
            .iter()
            .take(MAX_NOTES)
            .filter_map(|(_, path)| {
                let named = path.file_name()?.to_str()?;
                // A NOTE THAT WOULD NOT OPEN IS STILL A NOTE. It matched the
                // operator's own pattern and its clock puts it in the window,
                // so dropping it renders a night in which that finding never
                // existed; the mode, the race or the swap that stopped the read
                // is exactly what they would want to see.
                Some(match read_note(path, since, until) {
                    Some(contents) => pns::recap::noted(named, &contents),
                    None => pns::recap::unreadable(named),
                })
            })
            .collect(),
    })
}

/// Whether one name matches a pattern holding at most one `*`, which is the
/// only glob the config layer admits. Everything else is a literal, so a
/// pattern names one file.
fn matches_glob(name: &str, pattern: &str) -> bool {
    match pattern.split_once('*') {
        None => name == pattern,
        Some((head, tail)) => {
            name.len() >= head.len() + tail.len() && name.starts_with(head) && name.ends_with(tail)
        }
    }
}

/// One file's own clock, or None when it has none this can read.
fn modified_at(metadata: &std::fs::Metadata) -> Option<Duration> {
    metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()
}

/// Whether a clock puts a file inside the window, on `activity_in`'s half-open
/// rule and AT FULL PRECISION. Truncating to whole seconds excluded a file
/// written half a second after the marker and admitted one written half a
/// second after the window closed, which is the one edge each rule exists to
/// place.
fn within(at: Duration, since: u64, until: u64) -> bool {
    at > Duration::from_secs(since) && at <= Duration::from_secs(until)
}

/// One matched note read up to a ceiling, through a handle that is CHECKED
/// AFTER IT IS OPEN.
///
/// OPEN THEN VERIFY, because the scan and the read are two moments and a
/// directory other tools write into can change between them. The open refuses
/// to follow a link at all (`O_NOFOLLOW`), so a symlink dropped at a name the
/// glob matched cannot widen the read past the one directory the pattern names;
/// and the file type and the clock are re-read off the HANDLE, so a file
/// rewritten after the scan cannot feed this window contents from outside it.
/// Checking the path a second time instead would be the same race with more
/// steps: the answer would still describe whatever the name pointed at then.
///
/// LOSSY, for `run_bounded`'s reason: this is a plain file other tools write,
/// and one invalid byte must cost its own character rather than the note.
fn read_note(path: &Path, since: u64, until: u64) -> Option<String> {
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .ok()?;
    let metadata = file.metadata().ok()?;
    if !metadata.is_file() || !within(modified_at(&metadata)?, since, until) {
        return None;
    }
    let mut text = Vec::new();
    Read::take(&mut file, NOTE_READ_MAX)
        .read_to_end(&mut text)
        .ok()?;
    Some(String::from_utf8_lossy(&text).into_owned())
}

/// The listing tool, resolved through PATH. See `merged_pull_requests`.
const GH: &str = "gh";

/// How many merged pull requests one repository may contribute.
///
/// FIFTY, which is far past what an absence produces (ten in a ten-hour stretch
/// on this machine, MEASURED) and still a bound on somebody else's answer.
const GH_LIMIT: usize = 50;

/// How long the listing may take. THIRTY SECONDS, which is thirty times the
/// second the same call MEASURED today and short of anything a person would
/// call working. Nobody is waiting on it, so this exists to stop a wedged
/// network call holding the whole recap rather than to hurry a slow one.
const GH_DEADLINE: Duration = Duration::from_secs(30);

/// How much of the listing is read.
///
/// MEASURED AGAINST THE REAL WORKLOAD rather than against a service limit: the
/// last fifty merged pull requests of this repository come back as 187,965
/// bytes of JSON with a longest body of 9,672 characters, so the ordinary case
/// spends 37% of this. A pull request body may be far larger than that, and
/// enough of them at once take the section out for one window: past the cap the
/// JSON is truncated and fails to parse, which is the fail-closed direction
/// (half a listing is not a listing) but reads as "unavailable" with no hint
/// that size was the reason.
const GH_READ_MAX: u64 = 512 * 1024;

/// How many review notes one recap considers, and how much of each it reads.
const MAX_NOTES: usize = 25;

const NOTE_READ_MAX: u64 = 64 * 1024;

/// The configured command, handed the window on stdin, and what it said back
/// as timeline lines. None for every way of not answering.
///
/// ARGV STRAIGHT TO `Command`, NEVER THROUGH A SHELL, which is what makes the
/// key safe to hold anything: the words are the words, so there is no quoting
/// rule to get wrong and nothing in the window can be read as syntax.
///
/// THE SEAM IS THE ONE THE PROBES ALREADY USE. `run_bounded` writes the prompt
/// inside the deadline window, reads stdout lossily, kills the child when the
/// window closes and answers None on a non-zero exit, which is every rung of
/// this ladder but the last; `recap::answer` owns that one.
///
/// A BACKEND THAT IS NOT INSTALLED IS NOT A SPECIAL CASE. The spawn fails, the
/// seam answers None, and the recap posts the plain list saying so, which is
/// the same thing the operator sees when the model is simply slow.
///
/// AND NEITHER IS A SPENT BUDGET. An episode whose deadline is gone starts no
/// process at all: spawning one only to kill it on a zero-length window is a
/// model load nobody reads, and the plain list is already the answer.
fn summarize(argv: &[String], deadline: Duration, prompt: &str) -> Option<Vec<String>> {
    if deadline.is_zero() {
        return None;
    }
    let (program, arguments) = argv.split_first()?;
    let mut command = Command::new(program);
    command.args(arguments);
    pns::recap::answer(&run_bounded(
        command,
        Some(prompt),
        deadline,
        pns::recap::MAX_ANSWER_BYTES as u64 + 1,
    )?)
}

/// The window bounds off argv, or None for anything this will not vouch for.
///
/// EVERY UNKNOWN WORD IS A REFUSAL, never a silent default: a recap over a
/// window nobody asked for is worse than none. Both bounds are required, both
/// are plain counts through the crate's one numeric gate, and a window that
/// runs backwards is refused rather than read as empty.
fn recap_bounds(arguments: &[String]) -> Option<(u64, u64)> {
    let mut since = None;
    let mut until = None;
    let mut tokens = arguments.iter();
    while let Some(token) = tokens.next() {
        let bound = match token.as_str() {
            "--since" => &mut since,
            "--until" => &mut until,
            _ => return None,
        };
        // A REPEATED FLAG IS A REFUSAL TOO: two windows were asked for and only
        // one can be answered.
        if bound.is_some() {
            return None;
        }
        *bound = Some(pns::parse_count(tokens.next()?)?);
    }
    match (since, until) {
        (Some(since), Some(until)) if since <= until => Some((since, until)),
        _ => None,
    }
}

/// One epoch as the operator's own wall clock reads it, or a placeholder of the
/// same width when there is no readable time. ONE FUNCTION for the header's two
/// bounds and every timeline line, so the recap cannot render two clocks.
fn wall_clock(epoch: Option<u64>) -> String {
    epoch
        .and_then(local_minutes_since_midnight)
        .map(|minutes| format!("{:02}:{:02}", minutes / 60, minutes % 60))
        .unwrap_or_else(|| NO_WALL_CLOCK.to_string())
}

/// The recap posted, with the one fallback the locked spec names.
///
/// SYNCHRONOUS INSIDE THIS PROCESS, and REPORTING, which is the mode whose
/// whole purpose is that a failure is visible. Nobody is behind this, and a
/// silently dropped recap is the exact failure the feature exists to prevent.
///
/// THE FALLBACK IS A REAL MECHANISM. hermes answers 404 for a route it does not
/// know and 502 when the target rejects the delivery, and only a 2xx is
/// `delivered`, so a thread route the operator has not prepared refuses loudly.
/// The same body then goes to the default route with ONE line saying why it
/// landed there, which is the locked "falls back to a plain #pns message".
///
/// A VERDICT, NEVER A SENTENCE. The retry fires on `Failed` and `Unlaunched`
/// alone; `Silent` is an executable channel that RAN and has no second surface
/// to answer on, and reading it as a failure would post every recap twice on
/// every machine with a shell channel installed.
///
/// ACCEPTED LIMIT, AND IT IS THE SAME RULE'S OTHER SIDE: on a machine running
/// EXECUTABLE channels (`PNS_CHANNELS_DIR` set), `deliver` always answers
/// `Silent` for a channel that ran, whatever the gateway then said. So a 404
/// from an unprepared `pns-recap` route is invisible there and this fallback
/// never fires; the recap goes to the thread route and stays there. Closing it
/// would mean an executable channel reporting a per-destination outcome, which
/// is a change to the channel contract itself and not to a recap.
///
/// ONE FALLBACK AND NO LOOP. A default route that refuses too is a gateway
/// problem, and a recap is not worth a retry storm against one.
///
/// ACCEPTED LIMIT ON THE CHARACTER CEILING: the fallback line is appended to a
/// body `recap::fit` has already fitted, so the second post may exceed
/// `recap::MAX_CHARS` by that one line. Fitting it in would mean composing the
/// body twice, once per route, on a path taken only when the first route
/// refused. The ceiling has 100 characters of headroom under the gateway's own
/// split threshold and this line is 82 characters plus its newline, so the post
/// still lands as one message.
fn post_recap(body: &str, thread: bool, home: &str, hermes_key: Option<String>) -> i32 {
    if !thread {
        deliver_recap(body, "", home, hermes_key);
        return 0;
    }
    if !refused(&deliver_recap(body, RECAP_ROUTE, home, hermes_key.clone())) {
        return 0;
    }
    deliver_recap(
        &format!("{body}\n{THREAD_UNAVAILABLE}"),
        "",
        home,
        hermes_key,
    );
    0
}

/// One recap posted to one route, and what the route had to say about it.
///
/// IT SAYS WHAT HAPPENED, which is what `ReportMode::ReportOutcome` was for
/// and what it never actually did: `dispatch_legs` RETURNS its outcomes and
/// prints nothing, so the mode only ever moved the deadline. MEASURED against
/// a dead endpoint, `pns recap --since ... --until ...` printed nothing and
/// exited 0, which is exactly the drill an operator runs by hand to check a
/// `pns-recap` route they have just prepared, against exactly the failure it
/// is most likely to meet.
///
/// THE SAME LINE `run_event` PRINTS, prefix and all, because a second spelling
/// of one report is a second thing to keep in step. The detached child's
/// stdout is `/dev/null`, so this costs the event path nothing.
fn deliver_recap(
    body: &str,
    channel: &str,
    home: &str,
    hermes_key: Option<String>,
) -> Vec<(pns::routing::Leg, Delivery)> {
    // ONE LEG AND ONE DESTINATION, built by hand the way `doctor_mode` builds
    // its own: no decision was taken here, so there is no plan to derive legs
    // from. NOT DECORATIVE, because nothing about this was chosen to put
    // something in front of the operator; the card already did that.
    let leg = pns::routing::Leg {
        name: "hermes",
        mode: pns::routing::ReportMode::ReportOutcome,
        decorative: false,
    };
    let event = pns::args::EventArgs {
        agent: "pns".to_string(),
        state: "recap".to_string(),
        detail: body.to_string(),
        channel: channel.to_string(),
        ..Default::default()
    };
    // NO MOBILE VERDICT TO CARRY: the one leg is hermes, so the mobile table
    // was never read on this path and the default states exactly that.
    let outcomes = dispatch_legs(&[leg], false, &event, home, &Mobile::default(), hermes_key);
    for (leg, delivered) in &outcomes {
        if let Some(line) = delivered.clone().line_for(leg.mode) {
            println!("pns: {line}");
        }
    }
    outcomes
}

/// Whether a dispatch refused the recap, which is the only thing that earns
/// the fallback. See `post_recap`.
fn refused(outcomes: &[(pns::routing::Leg, Delivery)]) -> bool {
    outcomes
        .iter()
        .any(|(_, delivered)| matches!(delivered, Delivery::Failed(_) | Delivery::Unlaunched(_)))
}

/// The hermes route a threaded recap posts to. ONE CONST rather than a key: a
/// second machine wanting another name can have the key the day it exists, and
/// the operator prepares this route in hermes either way.
const RECAP_ROUTE: &str = "pns-recap";

/// The line the fallback adds, so a recap in the wrong place says why it is
/// there rather than looking like the design.
const THREAD_UNAVAILABLE: &str =
    "(the pns-recap route did not take this, so it landed on the default route instead)";

/// What a recap typed wrong is told.
const RECAP_USAGE: &str = "pns: usage: pns recap --since <epoch> --until <epoch>";

/// What a line shows for a moment whose clock could not be read: the same width
/// as a time, so the timeline still lines up.
const NO_WALL_CLOCK: &str = "--:--";

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

/// Where macOS keeps the Focus state, under the operator's own home.
const FOCUS_DB: &str = "Library/DoNotDisturb/DB";

/// One reading of the Focus store: the verdict the event path acts on, and
/// what the mode catalog beside it did.
///
/// THE CATALOG'S FAILURE RIDES OUT ON THE ANSWER rather than being read a
/// second time by the doctor. A second read is a second moment, and the doctor
/// would then be reporting on a file the decision never saw.
struct FocusReading {
    /// Whether a mode `[focus] silence` named is asserted right now.
    silenced: bool,
    /// Why the mode catalog could not be read, when it could not. `Some` means
    /// NO display name resolved, so only a raw `modeIdentifier` in the config
    /// could have matched anything.
    catalog: Option<std::io::ErrorKind>,
}

/// Whether a macOS Focus the config NAMED is asserted right now, or the error
/// the assertion store's own read failed with.
///
/// HOME-RELATIVE AND WITH NO ENV HATCH, deliberately. A variable naming this
/// path would let any producer force the answer in either direction, which is
/// the objection `Overrides::muted` already states about the mute. The test
/// seam is the sandbox's own `HOME`, which every binary test already sets.
///
/// NOTHING NAMED MEANS NOTHING READ. With no `[focus] silence` list there is
/// no mode an assertion could match, so the two files are never opened and the
/// default machine pays no IO for a feature it did not ask for.
///
/// `Err` IS "the store could not be read", and it exists for the doctor alone:
/// the event path reads it as not silenced, because this is a private,
/// undocumented Apple store that can change schema on any macOS update and a
/// reader that failed closed would silence every banner, card and pulse on the
/// morning after an upgrade. The doctor is the one place that says so out
/// loud, and the ERROR ITSELF is carried out rather than flattened, because a
/// store that is absent and a store that is gated send the operator to two
/// different places.
///
/// THE CATALOG'S OWN FAILURE IS NOT ONE OF THOSE. An unreadable
/// `ModeConfigurations.json` resolves no names, so only a raw `modeIdentifier`
/// in the config can still match: silencing less rather than more, which is
/// the same direction. It is reported rather than errored for exactly that
/// reason, and the doctor says it in a clause of its own.
///
/// READ THROUGH `readable_state_file` for the reasons that function states about
/// this tool's own files, which hold for a foreign one just as well: a FIFO at
/// the path would park the event forever, and a file some other hand grew is
/// otherwise learned about by allocating it. The live store is 6 KiB against
/// the existing 256 KiB ceiling.
fn focus_now(home: &str, silence: &[String]) -> std::io::Result<FocusReading> {
    if silence.is_empty() {
        return Ok(FocusReading {
            silenced: false,
            catalog: None,
        });
    }
    let store = Path::new(home).join(FOCUS_DB);
    let assertions =
        pns::system::readable_state_file(&store.join("Assertions.json"), RING_READ_MAX)?;
    let catalog =
        pns::system::readable_state_file(&store.join("ModeConfigurations.json"), RING_READ_MAX);
    Ok(FocusReading {
        silenced: pns::focus::silenced(
            &pns::focus::active_modes(&assertions),
            &pns::focus::mode_names(catalog.as_deref().unwrap_or_default()),
            silence,
        ),
        catalog: catalog.as_ref().err().map(std::io::Error::kind),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_test_support::*;
    #[test]
    fn a_hue_table_nobody_wrote_and_one_switched_off_are_different_reports() {
        // NO BRIDGE IS DIALLED BY ANY ROW HERE: every case answers before the
        // enabled-and-configured branch that makes the two GETs, which is the
        // only branch that touches a network.
        let lights = pns::config::Lights::default();
        assert!(
            matches!(
                lights_report(None, None, false),
                pns::doctor::LightsReport::Off
            ),
            "no [lights] table is off, whatever hue is doing"
        );
        assert!(
            matches!(
                lights_report(Some(&lights), None, false),
                pns::doctor::LightsReport::HueMissing
            ),
            "a table and NO [plugins.hue] at all is a config that is half written"
        );
        assert!(
            matches!(
                lights_report(Some(&lights), None, true),
                pns::doctor::LightsReport::HueDisabled
            ),
            "and a table beside a hue that IS written is a switch somebody turned \
             off, which is a decision rather than an omission"
        );
        assert!(
            matches!(
                lights_report(Some(&lights), Some(&toml::Table::new()), true),
                pns::doctor::LightsReport::NoBridge
            ),
            "an enabled hue naming no bridge dials nothing and says so"
        );
    }

    #[test]
    fn a_recap_window_is_two_plain_counts_in_either_order_and_nothing_else() {
        let bounds = |words: &[&str]| {
            recap_bounds(
                &words
                    .iter()
                    .map(|word| word.to_string())
                    .collect::<Vec<_>>(),
            )
        };
        assert_eq!(
            bounds(&["--since", "1756499000", "--until", "1756500000"]),
            Some((1_756_499_000, 1_756_500_000))
        );
        // Either order, because the spawner writes one and a hand run writes
        // whichever it likes.
        assert_eq!(
            bounds(&["--until", "1756500000", "--since", "1756499000"]),
            Some((1_756_499_000, 1_756_500_000))
        );
        // A window of one instant is a window: nothing happened in it, and the
        // body says so rather than the parser refusing to describe it.
        assert_eq!(bounds(&["--since", "5", "--until", "5"]), Some((5, 5)));
    }

    #[test]
    fn every_recap_window_this_will_not_vouch_for_is_refused_rather_than_defaulted() {
        // A RECAP OVER A WINDOW NOBODY ASKED FOR IS WORSE THAN NONE, so there
        // is no default half and no silent fallthrough: a missing bound, a
        // bound that is not a plain count, a window that runs backwards, a
        // repeated flag and any word this does not serve are each a refusal.
        let bounds = |words: &[&str]| {
            recap_bounds(
                &words
                    .iter()
                    .map(|word| word.to_string())
                    .collect::<Vec<_>>(),
            )
        };
        for refused in [
            vec![],
            vec!["--since", "1756499000"],
            vec!["--until", "1756500000"],
            vec!["--since", "1756500000", "--until", "1756499000"],
            vec!["--since", "yesterday", "--until", "1756500000"],
            vec!["--since", "-5", "--until", "1756500000"],
            vec!["--since", "1756499000", "--since", "1756499500"],
            vec!["--since", "1756499000", "--until", "1756500000", "--now"],
            vec!["--since", "1756499000", "--until"],
            vec!["1756499000", "1756500000"],
        ] {
            assert_eq!(bounds(&refused), None, "case: {refused:?}");
        }
    }

    #[test]
    fn a_glob_matches_only_what_its_own_two_ends_bracket() {
        // A STAR STANDS FOR ANYTHING INCLUDING NOTHING, and the ends are ends
        // rather than anywhere in the name.
        for (name, matches) in [
            ("checklist-s17.md", true),
            ("checklist-.md", true),
            ("checklist.md", false),
            ("checklist-s17.txt", false),
            ("other-s17.md", false),
        ] {
            assert_eq!(
                matches_glob(name, "checklist-*.md"),
                matches,
                "{name} against checklist-*.md"
            );
        }
        // AND THE TWO ENDS MAY NOT CLAIM THE SAME CHARACTERS. `notes-notes.md`
        // both starts with `notes-` and ends with `-notes.md`, sharing the one
        // hyphen between them, so a matcher asking only those two questions
        // would match a name too short to hold both ends at once.
        assert!(!matches_glob("notes-notes.md", "notes-*-notes.md"));
        assert!(matches_glob("notes--notes.md", "notes-*-notes.md"));
        // AND A PATTERN WITH NO `*` NAMES ONE FILE, which is the ordinary case
        // of an operator pointing at a single note.
        assert!(matches_glob("notes.md", "notes.md"));
        assert!(!matches_glob("notes.md.bak", "notes.md"));
    }

    #[test]
    fn a_note_is_judged_by_the_handle_it_was_opened_on_rather_than_by_its_name() {
        // THE SCAN AND THE READ ARE TWO MOMENTS, and this is the second one.
        // The directory belongs to the operator's other tools, so a name that
        // was a regular file inside the window when it was listed can be a
        // symlink out of that directory, or a file rewritten since, by the time
        // it is opened. CONSTRUCTED BY CALLING THE READ ITSELF, which is that
        // ordering: whatever the scan believed, this is what the read is handed.
        let directory = std::env::temp_dir().join(format!(
            "pns-note-read-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |since| since.as_nanos())
        ));
        std::fs::create_dir_all(&directory).expect("the scratch directory");
        let planted = |name: &str, at: Duration| {
            let path = directory.join(name);
            std::fs::write(&path, "# a finding\n").expect("the note");
            std::fs::File::options()
                .write(true)
                .open(&path)
                .expect("the note")
                .set_modified(std::time::UNIX_EPOCH + at)
                .expect("the note's clock");
            path
        };

        let inside = planted("checklist-inside.md", Duration::from_secs(1500));
        assert_eq!(
            read_note(&inside, 1000, 2000).as_deref(),
            Some("# a finding\n")
        );
        // THE WINDOW IS HALF-OPEN AT FULL PRECISION, on `activity_in`'s own
        // rule. Truncating to whole seconds put a note written half a second
        // after the marker outside the window and one written half a second
        // after it closed inside.
        let edge = planted("checklist-edge.md", Duration::from_millis(1_000_500));
        assert!(
            read_note(&edge, 1000, 2000).is_some(),
            "half a second past the near edge is inside the window"
        );
        let past = planted("checklist-past.md", Duration::from_millis(2_000_500));
        assert!(
            read_note(&past, 1000, 2000).is_none(),
            "half a second past the far edge is outside the window"
        );
        // A SYMLINK IS REFUSED RATHER THAN FOLLOWED, so a name swapped after
        // the scan cannot read a file the glob never named. The scan skips
        // links itself; this is what stops the one planted between the two.
        let swapped = directory.join("checklist-swapped.md");
        let _ = std::os::unix::fs::symlink(&inside, &swapped);
        assert!(
            read_note(&swapped, 1000, 2000).is_none(),
            "a symlink planted at a matched name was followed"
        );
        // AND A FILE REWRITTEN SINCE THE SCAN IS REFUSED for the same reason:
        // the clock on the handle is what decides, not the one the scan saw.
        std::fs::write(&inside, "# rewritten after the scan\n").expect("the rewrite");
        std::fs::File::options()
            .write(true)
            .open(&inside)
            .expect("the note")
            .set_modified(std::time::UNIX_EPOCH + Duration::from_secs(9000))
            .expect("the note's clock");
        assert!(
            read_note(&inside, 1000, 2000).is_none(),
            "a note rewritten after the scan was read into the window anyway"
        );
    }

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
