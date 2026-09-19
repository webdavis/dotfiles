//! The composition root: argv in, exit code out.
//!
//! This crate is responsible for decoding a command line into a request,
//! adapting stdin and stdout, constructing the concrete adapters and
//! registering them, invoking one use case, and translating its outcome into
//! operator-facing output and an exit status.
//!
//! It is responsible for no domain policy, state codec, filesystem
//! transaction, HTTP request, hook payload normalization, recap composition,
//! lighting algorithm or delivery implementation. Those are the things a
//! composition root historically accretes, so they are named here as the
//! things this file may not grow.
//!

pub mod legacy;
pub mod submit;

mod shell;
pub use shell::shell_mode;

mod home_report;
mod lights_command;

// CRATE-VISIBLE, because `report` now takes a `Paint` and a report's paint is
// this crate's own vocabulary. Nothing outside ever called it.
pub(crate) use home_report::{rows as home_report, setup_row as home_setup_row};
pub use lights_command::{LoopCommand, loop_command, quiet_command};

pub(crate) use std::collections::BTreeMap;
pub(crate) use std::path::Path;
pub(crate) use std::time::Duration;

pub(crate) use pns_adapters::select_plugins;
pub(crate) use pns_adapters::{BOT_TYPE, DiscordSettings, discord_backend};
pub(crate) use pns_adapters::{BRIDGE_DEADLINE, HuePulse, UreqBridge, quiet_window};

/// The hue settings with the pin's own refusal said out loud, the one way
/// every lamp caller reads it.
pub(crate) fn armed_hue_settings(settings: &toml::Table) -> Option<pns_adapters::HueSettings> {
    pns_adapters::armed_hue(settings, |refusal| eprintln!("{refusal}"))
}
pub(crate) use pns_adapters::{HermesKeys, hermes_keys};
pub(crate) use pns_adapters::{
    HookPayload, flattened, moshi_subcommand, parse_payload, transcript_reply,
};
pub(crate) use pns_adapters::{LoadOutcome, config_path, load_config};
pub(crate) use pns_adapters::{MOSHI_TYPE, mobile_backend, moshi_secret};
pub(crate) use pns_adapters::{SystemCommandRunner, SystemProbes, local_minutes_since_midnight};
pub(crate) use pns_domain::Delivery;
pub(crate) use pns_domain::Overrides;
pub(crate) use pns_domain::registry::roster;
pub(crate) use pns_domain::render;

mod certificate_notice;
mod channel_dispatch;
mod channel_settings;
mod command_daemon;
mod command_doctor;
mod command_enroll;
mod command_failures;
mod command_gateway;
mod command_github;
mod command_quiet_calendar;
pub(crate) use command_github::github_mode;
mod command_lights;
mod command_loop;
mod command_nag;
mod command_presence;
mod command_quiet;
mod command_recap;
mod command_setup;
mod command_stale;
mod command_tap;
mod daemon_runtime;
mod delivery_runtime;
mod doctor_home;
mod doctor_style;
mod event_flow;
mod failure_notice;
mod failures_page;
mod hook_dispatch;
mod hook_observations;
mod hook_payload;
mod invocation;
mod subcommand_usage;
mod tap_report;
use invocation::{arguments_after_subcommand, arguments_after_verb};
mod lamp_event_lease;
mod lamp_pulse;
mod lights_tick_runtime;
mod moshi_submission;
mod nag_schedule_runtime;
mod presence_runtime;
mod recap_delivery_runtime;
mod return_replay;
mod runtime_environment;
mod sender;
pub(crate) use pns_adapters::style;
mod turn_lifecycle;
mod turn_text;
mod wait_runtime;

pub(crate) use channel_settings::{
    Mobile, disabled_backend_warnings, plugin_settings, read_discord, read_mobile,
};
pub(crate) use command_daemon::{DAEMON_USAGE, daemon_mode};
pub(crate) use command_doctor::doctor_mode;
pub(crate) use command_failures::failures_mode;
pub(crate) use command_gateway::gateway_mode;
pub(crate) use command_lights::lights_mode;
pub(crate) use command_loop::loop_mode;
pub(crate) use command_nag::nag_mode;
pub(crate) use command_presence::presence_mode;
pub(crate) use command_quiet::{muted_now, quiet_mode};
pub(crate) use command_recap::recap_mode;
pub(crate) use command_setup::setup_mode;
pub(crate) use command_stale::stale_mode;
pub(crate) use daemon_runtime::daemon_run;
pub(crate) use event_flow::{Attempt, run_event};
pub(crate) use hook_dispatch::hook_mode;
pub(crate) use hook_observations::{
    arm_quota_stale_wait, config_change_detail, model_switch_detail, quota_observation_detail,
    record_policy_settings_change, sandbox_network_detail,
};
pub(crate) use hook_payload::{payload_is_whole, read_payload};
pub(crate) use lamp_event_lease::clear_held_lamps;
pub(crate) use lamp_pulse::{fire_pulse, fire_pulse_unless_quiet};
pub(crate) use lights_tick_runtime::lights_tick;
pub(crate) use moshi_submission::{blocking_event, gate_mode};
pub(crate) use nag_schedule_runtime::{NAG_OFF, arm_nag, clear_nag, nag_after_secs};
use pns_adapters::focus_now;
pub(crate) use pns_adapters::marker_files::renew_loop_lease;
pub(crate) use pns_adapters::marker_files::update_blocked_marker;
pub(crate) use pns_adapters::{MoshiApprovalForwarder, condense, spawn_recap};
pub(crate) use presence_runtime::{
    home_presence, last_narrowing, presence_snapshot, presence_status, system_probes,
};
pub(crate) use return_replay::replay_missed;
pub(crate) use runtime_environment::{
    env_deadline, executable_in_path, now_secs, overrides_from_env, state_dir,
};
pub(crate) use sender::{attribution, name_session};
pub(crate) use turn_lifecycle::{end_of_turn, failed_turn, named_project, start_of_turn};
pub(crate) use turn_text::turn_reply;
pub(crate) use wait_runtime::{end_blocked_wait, stale_after_secs, track_wait};

pub fn run() {
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

#[cfg(test)]
#[path = "tests/config/mod.rs"]
mod config;

#[cfg(test)]
#[path = "tests/decision_log/mod.rs"]
mod decision_log;

#[cfg(test)]
#[path = "tests/engine/mod.rs"]
mod engine;

#[cfg(test)]
#[path = "tests/home/mod.rs"]
mod home;

#[cfg(test)]
#[path = "tests/lights/mod.rs"]
mod lights;

#[cfg(test)]
#[path = "tests/routing/mod.rs"]
mod routing;
