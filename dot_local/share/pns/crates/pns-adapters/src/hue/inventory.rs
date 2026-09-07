use pns_domain::lamps::{Inventory, Lamp};

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
