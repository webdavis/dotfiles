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
const TARGET_KEYS: &str = "lights.<level>";
#[cfg(test)]
fn keys_of(table: &str) -> Option<&'static [&'static str]> {
    TABLE_KEYS
        .iter()
        .find(|(name, _)| *name == table)
        .map(|(_, keys)| *keys)
}

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
#[path = "config/contract_tests.rs"]
mod tests;
