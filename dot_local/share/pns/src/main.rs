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

pub(crate) use std::collections::BTreeMap;
pub(crate) use std::io::{IsTerminal, Read, Seek, Write};
pub(crate) use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
pub(crate) use std::os::unix::process::CommandExt;
pub(crate) use std::path::{Path, PathBuf};
pub(crate) use std::process::{Command, Stdio};
pub(crate) use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub(crate) use pns::args::parse_args;
pub(crate) use pns::channels::banner::BannerChannel;
pub(crate) use pns::channels::hermes::{
    DEFAULT_HERMES_URL, HermesChannel, UreqSignedPost, channel_url, hermes_secret, remote_deadline,
};
pub(crate) use pns::channels::hue::{
    BRIDGE_DEADLINE, HuePulse, TYPED_COMMAND_DEADLINE, UreqBridge, hue_settings, quiet_now,
    quiet_window,
};
pub(crate) use pns::channels::moshi::{
    DEFAULT_MOSHI_URL, MOSHI_TYPE, MoshiChannel, UreqPost, mobile_backend, moshi_secret,
    refused_backend_line,
};
pub(crate) use pns::channels::{Delivery, native_first};
pub(crate) use pns::config::{LoadOutcome, config_path, load_config};
pub(crate) use pns::engine::{Overrides, decide};
pub(crate) use pns::hooks::{
    HookPayload, condenser_prompt, condenser_verdict, flattened, moshi_subcommand, parse_payload,
    transcript_reply,
};
pub(crate) use pns::registry::{roster, select_plugins};
pub(crate) use pns::render;
pub(crate) use pns::system::{
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
mod command_quiet;
mod command_recap;
mod command_setup;
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
mod setup_publish_runtime;
mod setup_walk;
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
pub(crate) use command_quiet::{muted_now, quiet_mode};
pub(crate) use command_recap::recap_mode;
pub(crate) use command_setup::setup_mode;
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
pub(crate) use setup_publish_runtime::publish_config;
pub(crate) use setup_walk::walk;
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
