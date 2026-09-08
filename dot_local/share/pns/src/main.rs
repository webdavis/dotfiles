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
pub(crate) use std::path::Path;
pub(crate) use std::time::Duration;

pub(crate) use pns::channels::Delivery;
pub(crate) use pns::channels::hermes::hermes_secret;
pub(crate) use pns::channels::hue::{
    BRIDGE_DEADLINE, HuePulse, UreqBridge, hue_settings, quiet_window,
};
pub(crate) use pns::channels::moshi::{MOSHI_TYPE, mobile_backend, moshi_secret};
pub(crate) use pns::config::{LoadOutcome, config_path, load_config};
pub(crate) use pns::engine::Overrides;
pub(crate) use pns::registry::{roster, select_plugins};
pub(crate) use pns::render;
pub(crate) use pns::system::{SystemCommandRunner, SystemProbes, local_minutes_since_midnight};
pub(crate) use pns_adapters::{
    HookPayload, flattened, moshi_subcommand, parse_payload, transcript_reply,
};

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
mod daemon_runtime;
mod delivery_runtime;
mod event_flow;
mod hook_dispatch;
mod hook_observations;
mod hook_payload;
mod invocation;
mod lamp_event_lease;
mod lamp_pulse;
mod lights_tick_runtime;
mod moshi_submission;
mod nag_schedule_runtime;
mod presence_runtime;
mod recap_delivery_runtime;
mod return_replay;
mod runtime_environment;
mod turn_lifecycle;
mod turn_text;

pub(crate) use channel_settings::{
    Mobile, disabled_backend_warnings, plugin_settings, read_mobile,
};
pub(crate) use command_daemon::{DAEMON_USAGE, daemon_mode};
pub(crate) use command_doctor::doctor_mode;
pub(crate) use command_home::home_mode;
pub(crate) use command_lights::lights_mode;
pub(crate) use command_loop::loop_mode;
pub(crate) use command_nag::nag_mode;
pub(crate) use command_presence::presence_mode;
pub(crate) use command_pulse::pulse_mode;
pub(crate) use command_quiet::{muted_now, quiet_mode};
pub(crate) use command_recap::recap_mode;
pub(crate) use command_setup::setup_mode;
pub(crate) use daemon_runtime::daemon_run;
pub(crate) use event_flow::{Attempt, run_event};
pub(crate) use hook_dispatch::hook_mode;
pub(crate) use hook_observations::{
    arm_quota_stale_wait, config_change_detail, model_switch_detail, quota_observation_detail,
    record_policy_settings_change,
};
pub(crate) use hook_payload::{payload_is_whole, read_payload};
pub(crate) use lamp_event_lease::clear_held_lamps;
pub(crate) use lamp_pulse::{fire_pulse, fire_pulse_unless_quiet};
pub(crate) use lights_tick_runtime::lights_tick;
pub(crate) use moshi_submission::{blocking_event, gate_mode};
pub(crate) use nag_schedule_runtime::{NAG_OFF, arm_nag, clear_nag, nag_after_secs};
use pns_adapters::focus_now;
pub(crate) use pns_adapters::marker_files::renew_loop_lease;
pub(crate) use pns_adapters::marker_files::{end_blocked_wait, update_blocked_marker};
pub(crate) use pns_adapters::{MoshiApprovalForwarder, condense, git_branch, spawn_recap};
pub(crate) use presence_runtime::{
    home_presence, last_narrowing, presence_snapshot, presence_status, system_probes,
};
pub(crate) use return_replay::replay_missed;
pub(crate) use runtime_environment::{
    env_deadline, executable_in_path, now_secs, overrides_from_env, resolve_path, state_dir,
};
pub(crate) use turn_lifecycle::{end_of_turn, failed_turn, project_of, start_of_turn};
pub(crate) use turn_text::turn_reply;

fn main() {
    invocation::run();
}

pub(crate) use pns_adapters::enabled_hue_table;
pub(crate) use pns_domain::lamps::tick_bridge_deadline;

#[cfg(test)]
mod runtime_test_support;

#[cfg(test)]
pub(crate) use command_presence::{Polled, write_presence_reading};
#[cfg(test)]
pub(crate) use pns_adapters::LIGHTS_HELD;

#[cfg(test)]
mod lights_breath_runtime;
#[cfg(test)]
mod lights_tick_writes;
