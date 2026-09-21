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
#[cfg(test)]
mod http_script;

mod calendar;
pub use calendar::{read_calendar, read_calendar_state, write_calendar_state};

mod github;
mod install;
pub use install::{InstallSettings, install_settings, install_settings_of};
mod phone_marker;
mod tap_install;
pub use config::DaemonConfig;
pub use config::{
    BEHAVIOUR_WORDS, CalendarSource, Config, ConfigError, DEFAULT_ACK_DEADLINE,
    DEFAULT_BUSY_DEADLINE, Failures, GoogleCalendar, LoadOutcome, MAX_ARM_INTERVAL_SECS,
    MIN_ARM_INTERVAL_SECS, MOSHI_TYPE, PluginEntry, Presence, QuietCalendar, Recap, TABLE_KEYS,
    TOP_LEVEL, ack_deadline, armed_phone, config_path, enabled_hue_table, identity_placeholder,
    load_config, moshi_image_cards, moshi_secret, parse_config, parse_presence, phone_backend,
    remind_delay_range, render, strip_chezmoi_actions,
};
pub use config::{
    DEFAULT_POLL_SECS, DEFAULT_WEBHOOK_PORT, GITHUB, GithubSource, GithubWebhook, parse_github,
};
pub use github::client::{
    GITHUB_BODY_CAP, GITHUB_DEADLINE, GithubNotifications, Polled as GithubPolled,
};
pub use github::notifications::notification_threads;
pub use github::poll_state::{GITHUB_POLL_STATE, read_poll_state, write_poll_state};
pub use github::webhook::{
    Delivery, WEBHOOK_BODY_MAX, WEBHOOK_PATH, content_length, delivery, head_end,
};
pub use github::{GITHUB_EXTENSION, github_event, github_extensions};
pub use phone_marker::{
    MarkerReading, PhoneMarkerPath, TapFailure, phone_marker_path, read_phone_marker,
    record_phone_tap,
};
pub use tap_install::tap_install;

pub use config::{ROOM_MAX, room_fits};

pub use config::{
    RouterSettings, SetupFailure, device_identity, enabled_router_table, router_api_key,
    router_settings, stale_alert_route,
};

pub use config::banner_click;
pub use config::{DiscordSettings, armed_discord, discord_settings};
pub use config::{HermesKeys, hermes_keys};

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
pub use persistence::{QUIET_UNTIL, read_mute_expiry};
pub use persistence::{remember_staleness, remembered_staleness};

mod protocols;
pub use persistence::{LIGHTS_HELD, held_lamps, read_held, read_news, record_news, remember_held};
pub use protocols::markers as marker_files;
pub use protocols::markers::FileLoopLeases;
pub use protocols::{remind as remind_records, spool as job_spool};

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
    BRIDGE_DEADLINE, Bridge, DEFAULT_ROOMS, Enrollment, HuePulse, HueSettings, Mismatch,
    TYPED_COMMAND_DEADLINE, TypedLampBridge, UreqBridge, armed_hue, breath_arm_body,
    bridge_inventory, clear_body, clear_held, enroll, fade_body, grouped_light_ids_for_rooms,
    hue_settings, inventory, pulse_body, refused_mismatch, resolve_on_bridge, signal_fixtures,
    unreported_mismatch,
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
pub use macos::{
    LaunchdServiceController, SystemLaunchctlRunner, local_civil, local_epoch,
    local_minutes_since_midnight, local_timestamp, utc_timestamp,
};
pub use probes::SystemProbes;
pub use process::spawn_shell_event;
pub use process::{
    PROBE_READ_MAX, SystemCommandRunner, finish_bounded, run_bounded, run_bounded_reporting,
};

pub use destinations::banner::{
    BannerChannel, DEFAULT_TERMINAL_BUNDLE_ID, click_command, notifier_args, verbatim_argument,
};

pub use destinations::hermes::{
    DEFAULT_HERMES_URL, DEFAULT_REMOTE_DEADLINE_SECS, HermesChannel, channel_url, hermes_body,
    probe_route, probe_routes, remote_deadline,
};

pub use destinations::discord::{
    DiscordChannel, DiscordPost, DiscordReply, DiscordRequest, SessionThreads, UreqDiscordPost,
};

pub use destinations::moshi::{
    DEFAULT_MOSHI_UPLOAD_URL, DEFAULT_MOSHI_URL, HttpPost, MoshiChannel, POST_DEADLINE, UreqPost,
    herdr_link, refused_backend_line, webhook_body,
};

mod destinations;
pub use destinations::{ExecutableDestination, event_json, resolve_path};

mod unifi;
pub use unifi::{HomeStaleness, UniFiRouter, first_site_id, parse_clients};

pub use herdr::workspace_agent_statuses;
pub use herdr::{WorkspaceRow, parse_workspaces};

pub use presence::BridgePresencePoll;

pub use protocols::remind::FileRemindRecords;
pub use protocols::spool::FileJobSpool;

mod daemon_children;
pub use daemon_children::DaemonChildren;

mod daemon_shutdown;
pub use daemon_shutdown::{catch_termination, stopping};

pub use persistence::FileLampTick;

pub use herdr::HerdrWork;
pub use protocols::markers::FileLampMarkers;

mod recap;
pub use recap_document_wire::{Wire, read_mask};
mod recap_document_wire;
pub use recap::{
    ProcessSourceCommands, ProcessSummarizer, ReviewNotes, SummarizerFailure, git_facts,
    run_summarizer, transcript_excerpt,
};

mod doctor;
pub use doctor::{ANSWER_MAX, pairing_report};

pub use doctor::{daemon_heartbeat, doctor_bridge, hue_resolves, read_pairing, summarizer_report};
pub use process::{env_duration, moshi_hook_bin};

mod terminal;
pub use terminal::ConsoleTerminal;

pub use config::{SetupRenderer, compose_config};

pub use config::FileConfigPublisher;

mod codex;
mod git;
mod moshi_hook;
mod recap_card_wire;
mod recap_child;
pub use codex::summarize;
pub use git::{Checkout, git_checkout};
pub use moshi_hook::MoshiApprovalForwarder;
pub use recap_card_wire::{HandedCard, decode_handed_card};
pub use recap_child::{CARD_ON_STDIN, DURABLE, hand_recap_card, run_recap_bounded, spawn_recap};

pub use persistence::{
    ActivityEvent, DeliveryClaim, ImportFailure, SessionNote, SqliteStore, StoreError,
    StoredSummary,
};

mod harness;
pub use harness::{
    HookPayload, SessionFacts, flattened, is_harness_subcommand, moshi_subcommand, parse_payload,
    session_facts, transcript_reply,
};

#[cfg(test)]
mod state_fixtures;

/// How pns looks on a terminal, shared by the CLI and by the setup wizard's
/// terminal.
///
/// IT LIVES HERE RATHER THAN IN THE CLI because presentation on a terminal is a
/// concrete destination, and the wizard reaches its questions through this
/// crate's `Terminal`. With the vocabulary in the binary crate the wizard could
/// not use it, and the walk would have grown a second look.
pub mod style;
