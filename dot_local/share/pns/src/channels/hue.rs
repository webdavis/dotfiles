//! The hue channel, native: flash the configured rooms green or red for a
//! long command's exit code.
//!
//! Hue is config-SELECTED but not event-dispatched: the pulse fires on an
//! exit code from the long-command notifier, not on a notification, so this
//! channel is the binary's `pulse` mode reading the same `[plugins.hue]`
//! table (bridge, key, rooms) rather than a leg of the event plan.
//!
//! ONE PUT PER ROOM, and the bridge does the rest. It speaks CLIP v2 directly,
//! and the `on_off_color` signal it takes flashes a grouped light for a
//! duration and then puts the lamp back itself, so nothing here snapshots a
//! light, sequences a ramp or writes a restore. Every absence is a silent
//! no-op, and a failed pulse must never fail the caller.
//!
//! THE RESTORE IS MEASURED, not assumed. This paragraph used to assert it with
//! no source behind it, and the CLIP v2 specification says nothing either way
//! about what happens when a signal ends. The drill of 2026-09-01 put a signal
//! on a real lamp and read its full state back before and after, with the lamp
//! ON and again with it OFF: both times the bridge restored it byte for byte.
//! That is what this channel is built on, and it is why there is no snapshot
//! here and no restore engine anywhere.

// THE LAMP RESOLUTION POLICY moved to `pns-domain`, one file per question it
// answers. What stays here parses: the `[plugins.hue]` settings, the quiet
// window off a config string, and the bridge's own JSON listing.
pub use pns_domain::lamps::dim::{DimWindow, Showing, dim_showing};
pub use pns_domain::lamps::inventory::{
    Fixture, Inventory, Lamp, Missing, Unresolved, missing_sentence,
};
pub use pns_domain::lamps::mute::{Muting, mutable_names, muted_now};
pub use pns_domain::lamps::resolve::{LEVELS, Routed, Routing, remember, resolve};
pub use pns_domain::lamps::window::{QuietWindow, parse_window, quiet_now};

/// The rooms the bash pulsed when `HUE_PULSE_ROOMS` said nothing.
pub const DEFAULT_ROOMS: &[&str] = &["3F - Studio", "2F - Kitchen"];

/// Everything the pulse needs from the config, or None for the not-set-up
/// silence: a bridge and key are required, rooms come from the environment
/// override (newline-separated, room names carry spaces), else the settings
/// array, else the defaults.
#[derive(Debug, PartialEq)]
pub struct HueSettings {
    pub bridge: String,
    pub key: String,
    pub rooms: Vec<String>,
}

pub fn hue_settings(settings: &toml::Table, rooms_env: Option<&str>) -> Option<HueSettings> {
    let text = |key: &str| -> Option<String> {
        settings
            .get(key)?
            .as_str()
            .filter(|value| !value.is_empty())
            .map(String::from)
    };
    let from_env: Vec<String> = rooms_env
        .unwrap_or_default()
        .lines()
        .filter(|room| !room.is_empty())
        .map(String::from)
        .collect();
    Some(HueSettings {
        bridge: text("bridge")?,
        key: text("key")?,
        rooms: if from_env.is_empty() {
            settings
                .get("rooms")
                .and_then(|rooms| rooms.as_array())
                .map(|rooms| {
                    rooms
                        .iter()
                        .filter_map(|room| room.as_str())
                        .map(String::from)
                        .collect::<Vec<_>>()
                })
                .filter(|rooms| !rooms.is_empty())
                .unwrap_or_else(|| DEFAULT_ROOMS.iter().map(|room| room.to_string()).collect())
        } else {
            from_env
        },
    })
}

/// The window the operator configured, or None for no window at all.
///
/// A value that is not a `HH:MM-HH:MM` string is a REFUSAL rather than a
/// silent no-window: an operator who asked for quiet hours and mistyped them
/// would otherwise be flashed at 3am and told nothing.
pub fn quiet_window(settings: &toml::Table) -> Result<Option<QuietWindow>, String> {
    let Some(stated) = settings.get("quiet_hours") else {
        return Ok(None);
    };
    let Some(text) = stated.as_str() else {
        return Err(quiet_hours_refusal(stated.type_str()));
    };
    // EMPTY IS ABSENT, the rule the bridge and key beside it already follow.
    if text.is_empty() {
        return Ok(None);
    }
    parse_window(text)
        .map(Some)
        .ok_or_else(|| quiet_hours_refusal(&format!("{text:?}")))
}

/// The refusal, in the shape the config layer already refuses a setting by
/// name: what was written, and what it cost.
fn quiet_hours_refusal(offender: &str) -> String {
    format!("pns: config error (hue.quiet_hours is {offender}, not a HH:MM-HH:MM window); no pulse")
}

/// The `.data[]` array of a CLIP response, empty for anything unrecognized:
/// a bridge that answers with something this does not know is a no-op, never
/// a panic on a notification path.
fn data_entries(clip_json: &str) -> Vec<serde_json::Value> {
    serde_json::from_str::<serde_json::Value>(clip_json)
        .ok()
        .and_then(|body| Some(body.get("data")?.as_array()?.clone()))
        .unwrap_or_default()
}

/// The grouped_light ids of the wanted rooms, in WANTED order. A renamed room,
/// a room without a grouped_light, and unparseable JSON each drop out silently.
/// A room needs nothing else: the signal is one write to its group, and the
/// bridge restores it.
pub fn grouped_light_ids_for_rooms(rooms_json: &str, wanted: &[String]) -> Vec<String> {
    let rooms = data_entries(rooms_json);
    wanted
        .iter()
        .filter_map(|name| {
            rooms
                .iter()
                .filter(|room| {
                    room.pointer("/metadata/name")
                        .and_then(|found| found.as_str())
                        == Some(name.as_str())
                })
                .filter_map(|room| room.get("services")?.as_array())
                .flatten()
                .filter(|service| {
                    service.get("rtype").and_then(|kind| kind.as_str()) == Some("grouped_light")
                })
                .find_map(|service| service.get("rid")?.as_str().map(String::from))
        })
        .collect()
}

/// One lamp in the bridge's light listing, before the two joins run.
struct RawLamp {
    id: String,
    name: String,
    /// The DEVICE that owns it, which is what a ROOM lists as its child. The
    /// room join runs through here; the zone join does not.
    owner: String,
}

/// One listing entry's `metadata.name`.
fn named(entry: &serde_json::Value) -> Option<String> {
    Some(entry.pointer("/metadata/name")?.as_str()?.to_string())
}

/// The bridge's three listings, joined into one answer.
///
/// THE TWO JOINS ARE DIFFERENT SHAPES, measured against the CLIP v2 listings: a
/// room's `children` are DEVICE rids, so a lamp reaches its room through
/// `owner.rid`, while a zone's `children` are LIGHT rids and reach the lamp
/// directly. Writing one join for both would silently produce empty zones.
pub fn inventory(rooms_json: &str, lights_json: &str, zones_json: &str) -> Inventory {
    let raw: Vec<RawLamp> = data_entries(lights_json)
        .iter()
        .filter_map(|light| {
            Some(RawLamp {
                id: light.get("id")?.as_str()?.to_string(),
                name: light.pointer("/metadata/name")?.as_str()?.to_string(),
                owner: light.pointer("/owner/rid")?.as_str()?.to_string(),
            })
        })
        .collect();
    let rooms = data_entries(rooms_json);
    let zones = data_entries(zones_json);
    let mut lamps: Vec<Lamp> = raw
        .iter()
        .map(|lamp| Lamp {
            id: lamp.id.clone(),
            name: lamp.name.clone(),
            // FIRST ROOM WINS, and a lamp in two rooms is not a shape the bridge
            // produces: a light belongs to one room. Taking the last would make
            // the answer depend on listing order.
            room: rooms
                .iter()
                .find(|room| children_of(room).contains(&lamp.owner.as_str()))
                .and_then(named),
            zones: zones
                .iter()
                .filter(|zone| children_of(zone).contains(&lamp.id.as_str()))
                .filter_map(named)
                .collect(),
        })
        .collect();
    lamps.sort();
    Inventory {
        lamps,
        rooms: rooms.iter().filter_map(named).collect(),
        zones: zones.iter().filter_map(named).collect(),
    }
}

/// One listing entry's `children` rids, whatever kind they are.
fn children_of(entry: &serde_json::Value) -> Vec<&str> {
    entry
        .get("children")
        .and_then(|children| children.as_array())
        .map(|children| {
            children
                .iter()
                .filter_map(|child| child.get("rid")?.as_str())
                .collect()
        })
        .unwrap_or_default()
}

mod bridge;

pub use bridge::*;

#[cfg(test)]
mod fixtures;

#[cfg(test)]
mod settings_tests;

#[cfg(test)]
mod signal_tests;

#[cfg(test)]
mod routing_tests;

#[cfg(test)]
mod dim_tests;

#[cfg(test)]
mod body_tests;
