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
