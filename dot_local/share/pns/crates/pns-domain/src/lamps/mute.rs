//! Which places are muted right now, and which ones may be.

use super::inventory::{Inventory, Lamp};

/// What the operator's own by-hand mute is covering this second.
///
/// `Everything` IS THE FAIL DIRECTION AND NOT A COMMAND ANYBODY TYPES. A mute
/// record or a clock this run cannot read says nothing about which places are
/// quiet, and the direction every unreadable reading takes on a lamp path is
/// DARK: read as an empty list it was a house with every lamp loud, which is
/// the one outcome the operator armed the mute to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Muting {
    Everything,
    Places(Vec<String>),
}
/// Whether the operator's own by-hand mute covers this lamp.
///
/// EVERY NAME THE LAMP ANSWERS TO, which is the same vocabulary a declaration
/// names it by: `pns lights quiet "3F - Studio"` reaches every lamp in the
/// studio and `pns lights quiet "3F - Studio - HCL3"` reaches one. A zone name
/// works for the same reason.
pub fn muted_now(lamp: &Lamp, muting: &Muting) -> bool {
    let Muting::Places(muted) = muting else {
        return true;
    };
    std::iter::once(lamp.name.as_str())
        .chain(lamp.room.as_deref())
        .chain(lamp.zones.iter().map(String::as_str))
        .any(|name| muted.iter().any(|quiet| quiet == name))
}
/// Every name a mute may be typed at, sorted and deduplicated: the config's
/// declarations, plus the bridge's own lamps, rooms and zones.
///
/// THE VOCABULARY `pns lights quiet` ACCEPTS, and it is BOTH SOURCES because
/// the target grammar is lamp, room and zone rather than "whatever the config
/// happened to write down". Off the config alone it accepted a misspelled
/// declaration, which is a mute that can never match a lamp, and refused a real
/// inherited lamp the operator was reading off the bridge's own app, which is
/// the room they were standing in at the hour they wanted it quiet.
///
/// A BRIDGE THAT ANSWERED NOTHING LEAVES THE DECLARATIONS, which is the
/// direction that keeps the command usable with the transport down: those names
/// are the ones a mute can still enforce when it comes back.
pub fn mutable_names(
    lights: &crate::lamps::config::Lights,
    inventory: Option<&Inventory>,
) -> Vec<String> {
    let mut names: Vec<String> = lights
        .lamps
        .keys()
        .chain(lights.rooms.keys())
        .chain(lights.zones.keys())
        .cloned()
        .collect();
    if let Some(inventory) = inventory {
        names.extend(inventory.lamps.iter().map(|lamp| lamp.name.clone()));
        names.extend(inventory.rooms.iter().cloned());
        names.extend(inventory.zones.iter().cloned());
    }
    names.sort();
    names.dedup();
    names
}
