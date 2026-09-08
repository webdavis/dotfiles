use crate::parse_config;
use pns_domain::doctor::{
    Check, CheckKind, ConfigState, LightsReport, Outcome, Pairing, PairingReport, checks,
    exit_code, lights_lines, line, pairing_lines, summary,
};
use pns_domain::lamps as pns_hue;
use pns_domain::lamps::config::Behaviour;
use pns_domain::registry::{Registry, Selection, roster};
use pns_domain::{PresenceStatus, Unreadable};

use super::pairing_report;
/// Why a registered plugin was not checked: the config never switched it on.
const NOT_ENABLED: &str = "not enabled in the config";

/// Why it was not checked on a machine that has no config at all.
const NO_CONFIG: &str = "no config file, so only the core runs";

/// And why on one whose config could not be read.
const UNREADABLE_CONFIG: &str = "the config could not be read, so only the core runs";

/// Why a selected plugin was not checked: it is an input, and no leg can reach
/// it whatever the config says.
const A_SENSOR: &str = "a sensor and never a delivery destination";

/// The roster's own selection for a config, both halves the census takes.
fn census(config_text: &str) -> (Registry, Selection, Selection) {
    let registry = roster();
    let selected = registry
        .enabled(&parse_config(config_text).unwrap().plugin_switches())
        .unwrap();
    let registered = registry.all();
    (registry, registered, selected)
}

fn kind_for(config_text: &str, plugin: &str) -> CheckKind {
    let (_, registered, selected) = census(config_text);
    checks(&registered, &selected, ConfigState::Read)
        .into_iter()
        .find(|check| check.plugin == plugin)
        .unwrap_or_else(|| panic!("{plugin} is registered"))
        .kind
}

// --- the room sensor -----------------------------------------------------

/// `line` for a presence reading, which is the only outcome that check
/// takes.
fn presence_line_for(status: PresenceStatus) -> String {
    presence_line_with(status, None)
}

/// The same line, with whatever the narrowing ring last recorded.
fn presence_line_with(
    status: PresenceStatus,
    last_narrowing: Option<pns_domain::PresenceDecision>,
) -> String {
    line(
        &Check {
            plugin: "presence",
            kind: CheckKind::Presence,
        },
        &Outcome::Presence(status, last_narrowing),
    )
}

mod census;
mod lamps;
mod outcomes;
mod pairing_read;
mod pairing_render;
mod presence;
