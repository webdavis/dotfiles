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
//! duration and then restores the room itself, so nothing here snapshots a
//! light, sequences a ramp or writes a restore. Every absence is a silent
//! no-op, and a failed pulse must never fail the caller.

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

/// How long the bridge flashes before putting the room back, in milliseconds.
/// OPERATOR-TUNED against the live bridge on 2026-08-12 (trials 4 and 5): long
/// enough to catch from across the room, short enough not to be a light show.
const SIGNAL_DURATION_MS: u64 = 3000;

/// The grouped_light PUT body for one signal.
///
/// The bridge OWNS THE WHOLE EFFECT: it flashes the colour for the duration and
/// then restores the room to whatever it was, with no snapshot, no restore
/// writes and no choreography from us. That is why this channel is one PUT.
pub fn signal_body(color: crate::pulse::PulseColor) -> String {
    serde_json::json!({
        "signaling": {
            "signal": "on_off_color",
            "duration": SIGNAL_DURATION_MS,
            "colors": [{"xy": {"x": color.x, "y": color.y}}],
        },
    })
    .to_string()
}

/// The hue settings table, only when the plugin is ENABLED: the file
/// selects, and a disabled table must silence the pulse mode.
pub fn hue_enabled(config: &crate::config::Config) -> Option<&toml::Table> {
    config
        .plugins
        .get("hue")
        .filter(|hue| hue.enabled)
        .map(|hue| &hue.settings)
}

/// The bridge seam: authenticated GETs and PUTs against the CLIP paths.
pub trait Bridge {
    fn get(&self, path: &str) -> Option<String>;
    /// Fire and forget: `run` discards every outcome, so a bridge that
    /// refuses tells no one. Returning a result would be a seam with no
    /// consumer.
    fn put(&self, path: &str, body: &str);
}

/// The signal: one PUT per wanted room, and the bridge does the rest.
pub struct HuePulse<B: Bridge> {
    pub bridge: B,
    pub rooms: Vec<String>,
}

impl<B: Bridge> HuePulse<B> {
    pub fn run(&self, exit_code: &str) {
        let Some(rooms_json) = self.bridge.get("room") else {
            return;
        };
        let body = signal_body(crate::pulse::pulse_color(exit_code));
        // INDEPENDENT per group, and every outcome ignored: there is no shared
        // choreography left for a refused write to corrupt, so one room's
        // failure must not cost another its signal, and a failed pulse still
        // never fails the caller.
        for grouped_id in grouped_light_ids_for_rooms(&rooms_json, &self.rooms) {
            self.bridge
                .put(&format!("grouped_light/{grouped_id}"), &body);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Bridge, DEFAULT_ROOMS, HuePulse, grouped_light_ids_for_rooms, hue_enabled, hue_settings,
    };
    use std::cell::RefCell;

    const ROOMS_JSON: &str = r#"{"data":[
      {"id":"room-1","type":"room","metadata":{"name":"3F - Studio"},
       "children":[{"rid":"dev-a","rtype":"device"},{"rid":"dev-b","rtype":"device"}],
       "services":[{"rid":"grp-1","rtype":"grouped_light"}]},
      {"id":"room-2","type":"room","metadata":{"name":"2F - Kitchen"},
       "children":[{"rid":"dev-c","rtype":"device"}],
       "services":[{"rid":"grp-2","rtype":"grouped_light"}]}
    ]}"#;

    fn wanted(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| name.to_string()).collect()
    }

    // --- settings -----------------------------------------------------------

    fn table(text: &str) -> toml::Table {
        text.parse().unwrap()
    }

    #[test]
    fn a_bridge_and_key_are_required_and_their_absence_is_silence() {
        assert_eq!(hue_settings(&table(""), None), None);
        assert_eq!(
            hue_settings(&table(r#"bridge = "192.168.1.10""#), None),
            None
        );
        assert_eq!(hue_settings(&table(r#"key = "k""#), None), None);
        assert_eq!(
            hue_settings(&table("bridge = \"192.168.1.10\"\nkey = \"\""), None),
            None
        );
    }

    #[test]
    fn rooms_default_to_the_bash_pair_when_nothing_names_them() {
        let settings = hue_settings(&table("bridge = \"b\"\nkey = \"k\""), None).unwrap();
        assert_eq!(settings.rooms, DEFAULT_ROOMS.to_vec());
    }

    #[test]
    fn the_environment_override_wins_and_splits_on_newlines() {
        let settings = hue_settings(
            &table("bridge = \"b\"\nkey = \"k\"\nrooms = [\"Config Room\"]"),
            Some("Room A\nRoom B\n"),
        )
        .unwrap();
        assert_eq!(settings.rooms, vec!["Room A", "Room B"]);
    }

    #[test]
    fn the_settings_rooms_array_beats_the_defaults() {
        let settings = hue_settings(
            &table("bridge = \"b\"\nkey = \"k\"\nrooms = [\"Config Room\"]"),
            None,
        )
        .unwrap();
        assert_eq!(settings.rooms, vec!["Config Room"]);
    }

    // --- the CLIP parsers ---------------------------------------------------

    #[test]
    fn a_room_without_a_grouped_light_is_skipped_whole() {
        const NO_GROUP: &str = r#"{"data":[
          {"id":"room-1","type":"room","metadata":{"name":"Groupless"},
           "children":[{"rid":"dev-a","rtype":"device"}],"services":[]}
        ]}"#;
        assert!(grouped_light_ids_for_rooms(NO_GROUP, &wanted(&["Groupless"])).is_empty());
    }

    #[test]
    fn a_renamed_room_is_skipped_and_unparseable_json_is_empty() {
        assert!(grouped_light_ids_for_rooms(ROOMS_JSON, &wanted(&["Gone Room"])).is_empty());
        assert!(grouped_light_ids_for_rooms("not json", &wanted(&["3F - Studio"])).is_empty());
    }

    #[test]
    fn a_disabled_hue_table_silences_the_pulse_and_an_enabled_one_yields_its_settings() {
        use crate::config::parse_config;
        let disabled = parse_config("[plugins.hue]\nenabled = false\nbridge = \"b\"\n").unwrap();
        assert!(hue_enabled(&disabled).is_none());
        assert!(hue_enabled(&parse_config("").unwrap()).is_none());
        let enabled = parse_config("[plugins.hue]\nenabled = true\nbridge = \"b\"\n").unwrap();
        assert_eq!(
            hue_enabled(&enabled).and_then(|table| table.get("bridge")),
            Some(&toml::Value::String("b".to_string()))
        );
    }

    // --- the sequence -------------------------------------------------------

    struct ScriptedBridge {
        rooms: &'static str,
        gets: RefCell<Vec<String>>,
        puts: RefCell<Vec<(String, String)>>,
    }

    impl Bridge for ScriptedBridge {
        fn get(&self, path: &str) -> Option<String> {
            self.gets.borrow_mut().push(path.to_string());
            Some(self.rooms.to_string())
        }
        fn put(&self, path: &str, body: &str) {
            self.puts
                .borrow_mut()
                .push((path.to_string(), body.to_string()));
        }
    }

    fn bridge() -> ScriptedBridge {
        ScriptedBridge {
            rooms: ROOMS_JSON,
            gets: RefCell::new(Vec::new()),
            puts: RefCell::new(Vec::new()),
        }
    }

    fn pulse() -> HuePulse<ScriptedBridge> {
        HuePulse {
            bridge: bridge(),
            rooms: wanted(&["3F - Studio", "2F - Kitchen"]),
        }
    }

    // --- the signal ---------------------------------------------------------

    const RED_SIGNAL: &str = r#"{"signaling":{"colors":[{"xy":{"x":0.675,"y":0.322}}],"duration":3000,"signal":"on_off_color"}}"#;
    const GREEN_SIGNAL: &str = r#"{"signaling":{"colors":[{"xy":{"x":0.2151,"y":0.7106}}],"duration":3000,"signal":"on_off_color"}}"#;

    #[test]
    fn a_failure_signals_every_wanted_room_red_and_writes_nothing_else() {
        let hue = pulse();
        hue.run("1");
        assert_eq!(
            hue.bridge.puts.borrow().as_slice(),
            &[
                ("grouped_light/grp-1".to_string(), RED_SIGNAL.to_string()),
                ("grouped_light/grp-2".to_string(), RED_SIGNAL.to_string()),
            ],
            "one signal per wanted room, and no restore writes: the bridge owns the restore. \
             Independence is structural now: put reports nothing, so no outcome can \
             short-circuit the room behind it"
        );
        assert_eq!(
            hue.bridge.gets.borrow().as_slice(),
            &["room".to_string()],
            "the light inventory is never fetched: nothing here snapshots a light any more"
        );
    }

    #[test]
    fn a_success_signals_green() {
        let hue = pulse();
        hue.run("0");
        let puts = hue.bridge.puts.borrow();
        assert_eq!(puts.len(), 2);
        assert_eq!(puts[0].1, GREEN_SIGNAL);
        assert_eq!(puts[1].1, GREEN_SIGNAL);
    }

    #[test]
    fn a_room_the_bridge_does_not_have_is_skipped_in_silence() {
        let hue = HuePulse {
            bridge: bridge(),
            rooms: wanted(&["3F - Studio", "1F - Renamed Away"]),
        };
        hue.run("1");
        let puts = hue.bridge.puts.borrow();
        assert_eq!(
            puts.len(),
            1,
            "only the room that still exists is signalled"
        );
        assert_eq!(puts[0].0, "grouped_light/grp-1");
    }

    #[test]
    fn no_matching_rooms_or_no_lights_is_a_silent_no_op() {
        let hue = HuePulse {
            bridge: ScriptedBridge {
                rooms: r#"{"data":[]}"#,
                gets: RefCell::new(Vec::new()),
                puts: RefCell::new(Vec::new()),
            },
            rooms: wanted(&["3F - Studio"]),
        };
        hue.run("0");
        assert!(hue.bridge.puts.borrow().is_empty());
    }
}
