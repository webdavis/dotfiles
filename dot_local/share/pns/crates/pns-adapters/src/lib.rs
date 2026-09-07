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
pub use config::{
    BEHAVIOUR_WORDS, Config, ConfigError, DEFAULT_SUBMIT_DEADLINE_SECS, LoadOutcome,
    MAX_REFRESH_SECS, MIN_REFRESH_SECS, MOSHI_TYPE, PluginEntry, Presence, Recap, TABLE_KEYS,
    TOP_LEVEL, armed_mobile, config_path, identity_placeholder, load_config, mobile_backend,
    moshi_secret, parse_config, parse_presence, render, strip_chezmoi_actions, submit_deadline,
};

pub use config::{ROOM_MAX, room_fits};

pub use config::{
    RouterSettings, SetupFailure, device_identity, enabled_router_table, router_api_key,
    router_settings, stale_alert_channel,
};

pub use config::hermes_secret;

pub use config::select_plugins;

mod hue;
pub use hue::{
    BRIDGE_DEADLINE, Bridge, DEFAULT_ROOMS, HuePulse, HueSettings, Reading as HueReading,
    TYPED_COMMAND_DEADLINE, UreqBridge, breath_arm_body, clear_body, clear_held, fade_body,
    fixture_path, grouped_light_ids_for_rooms, held_render, hue_settings, inventory, pulse_body,
    pulse_render, quiet_window, resolve_on_bridge, signal_fixtures,
};

mod presence;
pub use presence::{
    PRESENCE_LOCK_FILE, PRESENCE_READ_MAX, PRESENCE_STATE_FILE, PresenceClaim, claim_presence_poll,
    instant_from_utc, parse_presence_line, poll_bridge_presence, read_bridge_presence,
    render_presence_line,
};

mod macos;
pub use macos::{FocusReading, focus_now};

mod persistence;
pub use persistence::{RING_READ_MAX, STATE_FILE_MODE, readable_state_file};

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
pub use destinations::{deliver_executable, event_json, native_first, resolve_path};

mod unifi;
pub use unifi::{UniFiRouter, first_site_id, parse_clients, read_home};

pub use herdr::workspace_agent_statuses;

pub use presence::BridgePresencePoll;

pub use persistence::publish_state_line;
