//! What every hue test builds from: the module's own items and the recorded
//! bridge answers. One copy, because these rows were one test module before
//! the file outgrew the size rule.

#![allow(unused_imports)]

pub use crate::channels::hue::{
    Bridge, DEFAULT_ROOMS, DimWindow, HuePulse, Inventory, Missing, Muting, QuietWindow, Routing,
    Showing, Unresolved, breath_arm_body, clear_body, clear_held, dim_showing, fade_body,
    grouped_light_ids_for_rooms, held_render, hue_settings, inventory, mutable_names, muted_now,
    parse_window, pulse_body, pulse_render, quiet_now, quiet_window, resolve, resolve_on_bridge,
};
pub use crate::config::Behaviour;
pub use std::cell::RefCell;

pub const ROOMS_JSON: &str = r#"{"data":[
  {"id":"room-1","type":"room","metadata":{"name":"3F - Studio"},
   "children":[{"rid":"dev-a","rtype":"device"},{"rid":"dev-b","rtype":"device"}],
   "services":[{"rid":"grp-1","rtype":"grouped_light"}]},
  {"id":"room-2","type":"room","metadata":{"name":"2F - Kitchen"},
   "children":[{"rid":"dev-c","rtype":"device"}],
   "services":[{"rid":"grp-2","rtype":"grouped_light"}]}
]}"#;

pub fn wanted(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| name.to_string()).collect()
}

pub fn bridge() -> ScriptedBridge {
    scripted(Some(ROOMS_JSON))
}

pub fn stock() -> Inventory {
    inventory(CLIP_ROOMS, CLIP_LIGHTS, CLIP_ZONES)
}

/// One config's `[lights]` table, written the way the parser answers it, so
/// a test states a config rather than a struct literal.
pub fn lights(written: &str) -> crate::config::Lights {
    *crate::config::parse_config(written)
        .expect("the test's own config parses")
        .lights
        .expect("and carries a lights table")
}

/// What one lamp ended up carrying, by name, so an assertion reads as the
/// operator's own vocabulary rather than as bridge ids.
pub fn carried(routing: &Routing, name: &str) -> Option<Vec<Behaviour>> {
    routing
        .lamps
        .iter()
        .find(|routed| routed.lamp.name == name)
        .map(|routed| routed.shows.clone())
}

pub fn table(text: &str) -> toml::Table {
    text.parse().unwrap()
}

pub struct ScriptedBridge {
    /// The room listing, or None for a bridge that answered nothing: an
    /// unreachable address, a key it refuses, a body that never arrived.
    pub rooms: Option<&'static str>,
    /// The light listing, answered ONLY to a caller that asked for it, so a
    /// test can see which GETs were really taken.
    pub lights: Option<&'static str>,
    pub zones: Option<&'static str>,
    pub gets: RefCell<Vec<String>>,
    pub puts: RefCell<Vec<(String, String)>>,
}
impl Bridge for ScriptedBridge {
    fn get(&self, path: &str) -> Option<String> {
        self.gets.borrow_mut().push(path.to_string());
        match path {
            "light" => self.lights.map(String::from),
            "zone" => self.zones.map(String::from),
            _ => self.rooms.map(String::from),
        }
    }
    fn put(&self, path: &str, body: &str) {
        self.puts
            .borrow_mut()
            .push((path.to_string(), body.to_string()));
    }
}
pub fn scripted(rooms: Option<&'static str>) -> ScriptedBridge {
    ScriptedBridge {
        rooms,
        lights: None,
        zones: None,
        gets: RefCell::new(Vec::new()),
        puts: RefCell::new(Vec::new()),
    }
}
// LIFTED FROM THE LIVE CAPTURE of 2026-08-20, trimmed to the rooms this
// repo's own map uses, with a zone added over two of the studio lamps.
// Every id, name and owner below the zone is the bridge's own.
// NO TEST HERE MAKES A CALL: the listings are literals.
pub const CLIP_ROOMS: &str = r#"{"data":[
  {"id":"b0957429-83c4-42d2-9ea1-016321cc34fa","type":"room",
   "metadata":{"name":"3F - Studio"},
   "children":[{"rid":"4f4abd00-253c-46de-86f8-d69a768d333f","rtype":"device"},
               {"rid":"5421a66a-c2ff-4e90-82b3-b762594d9d1d","rtype":"device"},
               {"rid":"c97b44a9-cdcc-48c3-a15d-630fdaa936d0","rtype":"device"},
               {"rid":"21699967-039c-4243-844f-13a9f6e23113","rtype":"device"}],
   "services":[{"rid":"14a9dd16-5751-49bc-b33e-701aecf1bce9","rtype":"grouped_light"}]},
  {"id":"09ccbfe8-6995-426e-b160-2e6dd866b59e","type":"room",
   "metadata":{"name":"2F - Kitchen"},
   "children":[{"rid":"b1e78057-aa81-4de0-ab08-6d06e1736dd6","rtype":"device"}],
   "services":[{"rid":"b8a1fbd9-829e-42b1-ae3e-c9f24bfcc882","rtype":"grouped_light"}]},
  {"id":"empty-room","type":"room","metadata":{"name":"3F - Cupboard"},
   "children":[],"services":[]}
]}"#;
/// The lights in the bridge's own listing order, which is NOT the order the
/// lamps are named in: HCL3 sits between HCL1 and HCL2.
///
/// The studio's fourth child owns nothing here on purpose: it is a Hue
/// dimmer SWITCH (`3F - Studio - HDS1`, verified in the same capture), so
/// the device join has a room member that is not a lamp in it.
pub const CLIP_LIGHTS: &str = r#"{"data":[
  {"id":"17295316-360e-4259-b8fd-928caf1f9c3e","type":"light",
   "owner":{"rid":"4f4abd00-253c-46de-86f8-d69a768d333f","rtype":"device"},
   "metadata":{"name":"3F - Studio - HCL1"}},
  {"id":"9d52d98c-76f0-47d8-a718-4b88cd123665","type":"light",
   "owner":{"rid":"c97b44a9-cdcc-48c3-a15d-630fdaa936d0","rtype":"device"},
   "metadata":{"name":"3F - Studio - HCL3"}},
  {"id":"de7b7231-1302-48ed-b0b5-9dd94763d350","type":"light",
   "owner":{"rid":"5421a66a-c2ff-4e90-82b3-b762594d9d1d","rtype":"device"},
   "metadata":{"name":"3F - Studio - HCL2"}},
  {"id":"0e7a5054-3720-4580-9d8e-8070216e9bfa","type":"light",
   "owner":{"rid":"b1e78057-aa81-4de0-ab08-6d06e1736dd6","rtype":"device"},
   "metadata":{"name":"2F - Kitchen - HCD3"}}
]}"#;
/// A ZONE LISTS ITS LIGHTS DIRECTLY, which is the join that differs from a
/// room's: a room's children are DEVICE rids and a zone's are LIGHT rids.
/// `Upstairs` and `Desk` deliberately overlap on HCL1, which is what makes
/// the same-level double cover reachable.
pub const CLIP_ZONES: &str = r#"{"data":[
  {"id":"zone-1","type":"zone","metadata":{"name":"Upstairs"},
   "children":[{"rid":"17295316-360e-4259-b8fd-928caf1f9c3e","rtype":"light"},
               {"rid":"de7b7231-1302-48ed-b0b5-9dd94763d350","rtype":"light"}]},
  {"id":"zone-2","type":"zone","metadata":{"name":"Desk"},
   "children":[{"rid":"17295316-360e-4259-b8fd-928caf1f9c3e","rtype":"light"}]},
  {"id":"zone-3","type":"zone","metadata":{"name":"Outdoors"},"children":[]}
]}"#;
