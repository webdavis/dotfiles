use super::{Check, CheckKind, ConfigState};
use crate::registry::{PluginKind, Registration, Selection};

/// Why a registered plugin was not checked: the config never switched it on.
const NOT_ENABLED: &str = "not enabled in the config";

/// Why it was not checked on a machine that has no config at all.
const NO_CONFIG: &str = "no config file, so only the core runs";

/// And why on one whose config could not be read.
const UNREADABLE_CONFIG: &str = "the config could not be read, so only the core runs";

/// Why a selected plugin was not checked: it is an input, and no leg can reach
/// it whatever the config says.
const A_SENSOR: &str = "a sensor and never a delivery destination";

/// One check per registration, in registration order, whatever the config
/// selected.
pub fn checks(registered: &Selection, selected: &Selection, config: ConfigState) -> Vec<Check> {
    registered
        .iter()
        .map(|entry| Check {
            plugin: entry.name,
            kind: kind_of(entry, selected, config),
        })
        .collect()
}

/// Why a plugin outside the selection was left out, in the words that are true
/// of THIS machine.
fn not_selected(config: ConfigState) -> &'static str {
    match config {
        ConfigState::Read => NOT_ENABLED,
        ConfigState::Absent => NO_CONFIG,
        ConfigState::Unreadable => UNREADABLE_CONFIG,
    }
}

/// What checking one registration means, given what the config selected.
///
/// NOT SELECTED IS ASKED FIRST, so a sensor the config never switched on reads
/// as absent by choice rather than as the kind it would have been.
fn kind_of(entry: &Registration, selected: &Selection, config: ConfigState) -> CheckKind {
    if !selected.iter().any(|chosen| chosen.name == entry.name) {
        return CheckKind::Skipped(not_selected(config));
    }
    match entry.kind {
        // THE ONE SENSOR WITH SOMETHING TO REPORT. Nothing is sent to it and
        // nothing ever will be, but its reading is the one thing about it an
        // operator cannot see any other way, and a bare `skipped, a sensor`
        // line would leave a machine whose bridge stopped answering looking
        // exactly like one that is fine.
        PluginKind::Sensor if entry.name == crate::registry::PRESENCE => CheckKind::Presence,
        PluginKind::Sensor => CheckKind::Skipped(A_SENSOR),
        // A channel the binary drives in its own mode is checkable, just not
        // as a leg: no event routes to it, so a send would never happen and
        // reporting it as skipped would hide the one destination hardest to
        // verify any other way.
        PluginKind::Channel(routing) if !routing.event_dispatched => CheckKind::Pulse,
        PluginKind::Channel(_) => CheckKind::Send,
    }
}
