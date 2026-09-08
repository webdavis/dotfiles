//! The concrete edges: one module per real capability, never one broad
//! infrastructure module.
//!
//! This crate is responsible for implementing the ports pns-application
//! declares, against the actual things they stand for: the notification
//! destinations, the Hue attention indicator, the UniFi home-probe source, the
//! macOS idle and lock readers, the mosh terminal reader, the herdr view
//! reader, the filesystem protocols that remain protocols, SQLite persistence,
//! configuration loading and rendering, bounded process execution, the recap
//! sources and the summary providers.
//!
//! It is responsible for no policy. An adapter reports what it observed or
//! what a delivery did; what that means is decided in pns-domain, and which
//! order it happens in is decided in pns-application.
//!
//! Configuration parsing, backend settings and rendering live here.

mod config;
pub use config::DaemonConfig;
pub use config::{
    BEHAVIOUR_WORDS, Config, ConfigError, DEFAULT_SUBMIT_DEADLINE_SECS, LoadOutcome,
    MAX_REFRESH_SECS, MIN_REFRESH_SECS, MOSHI_TYPE, PluginEntry, Presence, Recap, TABLE_KEYS,
    TOP_LEVEL, armed_mobile, config_path, enabled_hue_table, identity_placeholder, load_config,
    mobile_backend, moshi_secret, parse_config, parse_presence, render, strip_chezmoi_actions,
    submit_deadline,
};

pub use config::{ROOM_MAX, room_fits};

pub use config::{
    RouterSettings, SetupFailure, device_identity, enabled_router_table, router_api_key,
    router_settings, stale_alert_channel,
};

pub use config::hermes_secret;

pub use config::select_plugins;

mod persistence;
pub use persistence::FileLampState;
pub use persistence::lights_codec;
pub use persistence::{
    ACTIVITY, ACTIVITY_KEPT, ACTIVITY_MAX_CHARS, ACTIVITY_READ_MAX, DECISIONS, FileRecords,
    MISSED_NOTIFICATIONS,
};
pub use persistence::{
    HeldLock, RING_READ_MAX, STATE_FILE_MODE, append_ring_line, claim_lock, decision_codec,
    journal_codec, now_secs, presence_journal, publish_state_line, readable_state_file, state_dir,
};
pub use persistence::{QUIET_UNTIL, read_quiet_expiry};
pub use persistence::{remember_staleness, remembered_staleness};

mod protocols;
pub use persistence::{LIGHTS_HELD, held_lamps, read_held, read_news, record_news, remember_held};
pub use protocols::markers as marker_files;
pub use protocols::markers::FileLoopLeases;
pub use protocols::{nag as nag_records, spool as job_spool};

pub use persistence::LIGHTS_SAID;
pub use protocols::return_window;
pub use protocols::turn_markers;

pub use protocols::config_publication;

pub use persistence::{
    LIGHTS_QUIET, LIGHTS_QUIET_SAID, advance_streak, muted_state, publish_muted,
};

pub use persistence::record_policy_settings_change;

mod hue;
pub use hue::{
    BRIDGE_DEADLINE, Bridge, DEFAULT_ROOMS, HuePulse, HueSettings, TYPED_COMMAND_DEADLINE,
    TypedLampBridge, UreqBridge, breath_arm_body, bridge_inventory, clear_body, clear_held,
    fade_body, grouped_light_ids_for_rooms, hue_settings, inventory, pulse_body, quiet_window,
    resolve_on_bridge, signal_fixtures,
};

mod presence;
pub use presence::{
    PRESENCE_LOCK_FILE, PRESENCE_READ_MAX, PRESENCE_STATE_FILE, PresenceClaim, claim_presence_poll,
    instant_from_utc, parse_presence_line, poll_bridge_presence, read_bridge_presence,
    render_presence_line,
};

mod macos;
pub use macos::{FocusReading, focus_now};

mod herdr;
mod probes;
mod process;
pub use macos::{local_minutes_since_midnight, utc_timestamp};
pub use probes::SystemProbes;
pub use process::{PROBE_READ_MAX, SystemCommandRunner, finish_bounded, run_bounded};

pub use destinations::banner::{
    BannerChannel, DEFAULT_TERMINAL_BUNDLE_ID, click_command, notifier_args, verbatim_argument,
};

pub use destinations::hermes::{
    DEFAULT_HERMES_URL, HermesChannel, channel_url, hermes_body, remote_deadline,
};

pub use destinations::moshi::{
    DEFAULT_MOSHI_URL, HttpPost, MoshiChannel, POST_DEADLINE, UreqPost, herdr_link,
    refused_backend_line, webhook_body,
};

mod destinations;
pub use destinations::{ExecutableDestination, event_json, resolve_path};

mod unifi;
pub use unifi::{HomeStaleness, UniFiRouter, first_site_id, parse_clients};

pub use herdr::workspace_agent_statuses;

pub use presence::BridgePresencePoll;

pub use protocols::nag::FileNagRecords;
pub use protocols::spool::FileJobSpool;

mod daemon_children;
pub use daemon_children::DaemonChildren;

pub use persistence::FileLampTick;

pub use herdr::HerdrWork;
pub use protocols::markers::FileLampMarkers;

mod recap;
pub use recap::{GitHubMerges, ProcessSummarizer, ReviewNotes};

mod doctor;
pub use doctor::{ANSWER_MAX, pairing_report};

pub use doctor::{daemon_heartbeat, doctor_bridge, hue_resolves, read_pairing};
pub use process::{env_deadline, moshi_hook_bin};

mod terminal;
pub use terminal::ConsoleTerminal;

pub use config::{SetupRenderer, compose_config};

pub use config::FileConfigPublisher;

mod codex;
mod git;
mod moshi_hook;
mod recap_child;
pub use codex::condense;
pub use git::git_branch;
pub use moshi_hook::MoshiApprovalForwarder;
pub use recap_child::{run_recap_bounded, spawn_recap};

pub use persistence::{DeliveryClaim, ImportFailure, SqliteStore, StoreError};

#[cfg(test)]
mod state_fixtures;
