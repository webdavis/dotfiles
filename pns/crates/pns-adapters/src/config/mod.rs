//! The config edge: `~/.config/pns/config.toml` decides which plugins run.
//!
//! The file SELECTS; it never defines. Every plugin is compiled in, disabled
//! until its table says `enabled = true`, so a machine runs exactly what its
//! config names and nothing else. The settings inside a plugin's table are
//! free-form here: this layer proves the shape, the registry interprets the
//! contents, and neither knows the other's plugin names.
//!
//! The top-level policy tables are read by this layer rather than a plugin.
//! Their booleans, thresholds, command arguments, Focus and delivery-class
//! names, and lamp settings are validated here. Unknown keys, invalid counts
//! and malformed command lists are refused rather than passed through.
//!
//! Failure directions, each pinned by a test: a MALFORMED file is a loud
//! error and never a silent empty config, because a typo that turns every
//! notification off must not pass quietly; a MISSING file is its own honest
//! outcome, distinct from both error and emptiness, so the caller can say
//! "unconfigured" instead of guessing; unknown top-level keys are refused,
//! so `[plugin.hue]` cannot silently disable what `[plugins.hue]` enables.

use pns_domain::lamps::config::{
    Behaviour, Blocked, Breath, BreatheThenFlare, Lights, Looping, Pulse, Target, Unread,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
mod model;
pub use model::{Config, ConfigError, LoadOutcome, PluginEntry};
mod load;
pub use load::{config_path, load_config, parse_config};
mod plugins;
pub use plugins::{DEFAULT_SUBMIT_DEADLINE_SECS, armed_mobile, enabled_hue_table, submit_deadline};
mod recap;
pub use pns_domain::recap::Recap;
use recap::{MAX_SUMMARIZER_DEADLINE_SECS, parse_recap};
mod recap_values;
use recap_values::{argv, seconds, threshold};
mod recap_sources;
use recap_sources::{note_glob, repositories};
mod delivery;
use delivery::parse_delivery;
mod focus;
use focus::parse_focus;
mod daemon;
mod retry;
pub use daemon::DaemonConfig;
use daemon::{DEFAULT_DAEMON_ENABLED, parse_daemon};
mod nag;
use nag::{NAG_OFF, backstop_outlasts_the_nag, parse_nag};
mod failures;
pub use failures::Failures;
use failures::parse_failures;
mod values;
use values::{bounded, flag, strings, text};
mod schema;
pub use schema::{TABLE_KEYS, TOP_LEVEL};
use schema::{TARGET_KEYS, admits, admits_flat, keys_of, unknown_key};
mod lights_tables;
use lights_tables::parse_lights;
mod lights_bounds;
use lights_bounds::{
    MAX_FADE_MS, MAX_GIVE_UP_AFTER_SECS, MAX_THRESHOLD_SECS, MIN_FADE_MS, MIN_LEASE_TIMEOUT_SECS,
    MIN_THRESHOLD_SECS, accent_agrees, behaviour_table, breath_key, ends_agree, percent,
};
pub use lights_bounds::{MAX_REFRESH_SECS, MIN_REFRESH_SECS};
mod lights_targets;
use lights_targets::parse_targets;
pub use pns_domain::lamps::config::BEHAVIOUR_WORDS;
mod presence;

pub use presence::parse_presence;

mod mobile;
pub use mobile::{MOSHI_TYPE, mobile_backend, moshi_secret};

mod render;
pub use render::{identity_placeholder, render, strip_chezmoi_actions};

mod room;
pub use room::{ROOM_MAX, room_fits};
mod presence_values;
pub use presence_values::Presence;
use presence_values::{
    DEFAULT_DESK_STALE_AFTER_SECS, DEFAULT_PRESENCE_POLL_SECS, DEFAULT_PRESENCE_STALE_AFTER_SECS,
    MAX_DESK_STALE_AFTER_SECS, MAX_PRESENCE_POLL_SECS, MIN_PRESENCE_POLL_SECS, PRESENCE_TYPE,
};

mod router;
pub use router::{
    RouterSettings, SetupFailure, device_identity, enabled_router_table, router_api_key,
    router_settings, stale_alert_channel,
};

mod banner;
pub use banner::banner_click;

mod hermes;
pub use hermes::hermes_secret;

mod selection;
pub use selection::select_plugins;

#[cfg(test)]
mod tests;

/// How many `key = value` pairs a config-shaped text documents, commented
/// lines included, having checked each one against the roster row of the
/// heading above it.
///
/// THE SCAN READS THE COMMENTED LINES TOO, which is the half a parse cannot
/// reach: most of a documented config is documentation, and a key documented
/// there but refused by the code is a line an operator uncomments and then
/// cannot load.
///
/// The renderer and the remaining root setup/template tests temporarily
/// carry this same private scanner. Steps 13.6 and 13.7 remove the root copy
/// when those consumers move. The count is returned because only the shipped
/// template has a number worth pinning.
///
/// WHITESPACE-EXACT in two places (`# ` and ` = `), which is what the
/// template's own count is a fence around: a text writing `key= value` on a run
/// of lines drops exactly that run and nothing else says so.
#[cfg(test)]
pub(crate) fn documented_keys_the_roster_serves(text: &str) -> usize {
    let mut table = String::new();
    let mut found = 0;
    for line in text.lines() {
        let bare = line.strip_prefix("# ").unwrap_or(line);
        if let Some(heading) = bare
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
        {
            table = heading.to_string();
            continue;
        }
        let Some((key, _)) = bare.split_once(" = ") else {
            continue;
        };
        if !key
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            || key.is_empty()
        {
            continue;
        }
        // A nested table carries the operator's own name; the roster holds
        // the prefix, the way the refusals do.
        let roster_table = match table.split('.').collect::<Vec<_>>()[..] {
            ["lights", "lamp" | "room" | "zone", ..] => TARGET_KEYS.to_string(),
            _ => table.clone(),
        };
        let serves = keys_of(&roster_table)
            .unwrap_or_else(|| panic!("it writes `[{table}]`, which no table serves"));
        assert!(
            serves.contains(&key),
            "it documents `{key}` under `[{table}]`, which does not serve it"
        );
        found += 1;
    }
    found
}

#[cfg(test)]
mod router_tests;

#[cfg(test)]
mod selection_tests;

mod setup;
pub use setup::{SetupRenderer, compose_config};

mod setup_publisher;
pub use setup_publisher::FileConfigPublisher;
