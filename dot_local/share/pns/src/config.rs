//! The config edge: `~/.config/pns/config.toml` decides which plugins run.
//!
//! The file SELECTS; it never defines. Every plugin is compiled in, disabled
//! until its table says `enabled = true`, so a machine runs exactly what its
//! config names and nothing else. The settings inside a plugin's table are
//! free-form here: this layer proves the shape, the registry interprets the
//! contents, and neither knows the other's plugin names.
//!
//! `[recap]`, `[focus]`, `[daemon]` and `[lights]` are the four top-level
//! tables that are not plugins: four booleans, two counts, one argument list,
//! one list of Focus mode names and the lamp policy's own scalars and maps,
//! all read by THIS layer. Because it reads them, it can judge them, so an
//! unknown key inside any of them, a count that is not a threshold, and a
//! summarizer that is not a list of command words are refused rather than
//! passed along the way a plugin's settings are.
//!
//! Failure directions, each pinned by a test: a MALFORMED file is a loud
//! error and never a silent empty config, because a typo that turns every
//! notification off must not pass quietly; a MISSING file is its own honest
//! outcome, distinct from both error and emptiness, so the caller can say
//! "unconfigured" instead of guessing; unknown top-level keys are refused,
//! so `[plugin.hue]` cannot silently disable what `[plugins.hue]` enables.

pub use pns_adapters::{
    BEHAVIOUR_WORDS, Config, ConfigError, DEFAULT_SUBMIT_DEADLINE_SECS, LoadOutcome,
    MAX_REFRESH_SECS, MIN_REFRESH_SECS, PluginEntry, Presence, Recap, TABLE_KEYS, TOP_LEVEL,
    armed_mobile, config_path, identity_placeholder, load_config, parse_config, parse_presence,
    strip_chezmoi_actions, submit_deadline,
};
pub use pns_domain::lamps::config::{
    Behaviour, Blocked, Breath, BreatheThenFlare, DEFAULT_BLOCKED,
    DEFAULT_BLOCKED_GIVE_UP_AFTER_SECS, DEFAULT_DIM, DEFAULT_DONE, DEFAULT_FAILED,
    DEFAULT_LEASE_TIMEOUT_SECS, DEFAULT_LOOP_MOTION, DEFAULT_LOOP_THRESHOLD_SECS,
    DEFAULT_REFRESH_SECS, DEFAULT_UNREAD_AFTER_SECS, DEFAULT_UNREAD_BREATH, Lights, Looping, Pulse,
    Target, Unread,
};
#[cfg(test)]
fn keys_of(table: &str) -> Option<&'static [&'static str]> {
    TABLE_KEYS
        .iter()
        .find(|(name, _)| *name == table)
        .map(|(_, keys)| *keys)
}

#[cfg(test)]
#[path = "config/contract_tests.rs"]
mod tests;
