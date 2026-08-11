//! The hue channel, native: pulse the configured rooms green or red for a
//! long command's exit code, then restore every light exactly.
//!
//! Hue is config-SELECTED but not event-dispatched: the pulse fires on an
//! exit code from the long-command notifier, not on a notification, so this
//! channel is the binary's `pulse` mode reading the same `[plugins.hue]`
//! table (bridge, key, rooms) rather than a leg of the event plan.
//!
//! The bridge speaks CLIP v2 directly, replacing the openhue CLI: rooms name
//! their grouped_light service and their member devices, lights name their
//! owning device, and the snapshot mirrors the bash TSV so the restore logic
//! stays the decision the R1 pulse module already pinned. Every absence is a
//! silent no-op, and a failed pulse must never fail the caller.

use std::time::Duration;

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

/// The `rid`s of one rtype, out of one array key, for each wanted room in
/// WANTED order. Shared by the two room walks, which differ only in which
/// key and rtype they are after.
fn room_rids(rooms_json: &str, wanted: &[String], array_key: &str, rtype: &str) -> Vec<String> {
    let rooms = data_entries(rooms_json);
    wanted
        .iter()
        .flat_map(|name| {
            rooms
                .iter()
                .filter(|room| {
                    room.pointer("/metadata/name")
                        .and_then(|found| found.as_str())
                        == Some(name.as_str())
                })
                .filter_map(|room| room.get(array_key)?.as_array())
                .flatten()
                .filter(|entry| entry.get("rtype").and_then(|kind| kind.as_str()) == Some(rtype))
                .filter_map(|entry| entry.get("rid")?.as_str().map(String::from))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// The grouped_light service ids of the wanted rooms, in wanted order; a
/// configured room the bridge no longer knows is skipped, never fatal.
pub fn grouped_light_ids(rooms_json: &str, wanted: &[String]) -> Vec<String> {
    room_rids(rooms_json, wanted, "services", "grouped_light")
}

/// The device ids belonging to the wanted rooms: what maps a light back to
/// its room, because lights name their owning device, not their room.
pub fn room_device_ids(rooms_json: &str, wanted: &[String]) -> Vec<String> {
    room_rids(rooms_json, wanted, "children", "device")
}

/// One light's snapshot, mirroring the bash TSV: mode is `ct` when the
/// mirek reading is valid, else `xy`.
#[derive(Debug, PartialEq)]
pub struct LightState {
    pub id: String,
    pub on: bool,
    pub brightness: f64,
    pub mode: String,
    pub v1: String,
    pub v2: String,
}

pub fn light_snapshot(lights_json: &str, device_ids: &[String]) -> Vec<LightState> {
    data_entries(lights_json)
        .iter()
        .filter(|light| {
            light
                .pointer("/owner/rid")
                .and_then(|owner| owner.as_str())
                .is_some_and(|owner| device_ids.iter().any(|device| device == owner))
        })
        .filter_map(|light| {
            let rendered = |path: &str| {
                light
                    .pointer(path)
                    .map(|value| value.to_string())
                    .unwrap_or_default()
            };
            let ct = light.pointer("/color_temperature/mirek_valid")
                == Some(&serde_json::Value::Bool(true));
            Some(LightState {
                id: light.get("id")?.as_str()?.to_string(),
                on: light
                    .pointer("/on/on")
                    .and_then(|on| on.as_bool())
                    .unwrap_or(false),
                // The bash jq defaulted an absent dimming to 100 rather than
                // restoring a light to darkness.
                brightness: light
                    .pointer("/dimming/brightness")
                    .and_then(|brightness| brightness.as_f64())
                    .unwrap_or(100.0),
                mode: if ct { "ct" } else { "xy" }.to_string(),
                v1: if ct {
                    rendered("/color_temperature/mirek")
                } else {
                    rendered("/color/xy/x")
                },
                v2: if ct {
                    String::new()
                } else {
                    rendered("/color/xy/y")
                },
            })
        })
        .collect()
}

/// A CLIP numeric field out of one of our own rendered strings.
fn number(raw: &str) -> f64 {
    raw.parse().unwrap_or_default()
}

/// The grouped_light PUT body for one pulse step: on, the color, the
/// brightness, and the 1200ms ramp.
pub fn pulse_body(x: &str, y: &str, brightness: &str) -> String {
    serde_json::json!({
        "on": {"on": true},
        "color": {"xy": {"x": number(x), "y": number(y)}},
        "dimming": {"brightness": number(brightness)},
        "dynamics": {"duration": PULSE_TRANSITION.as_millis() as u64},
    })
    .to_string()
}

/// Each ramp finishes before the next step starts, so the sleep and the
/// transition are the same number by construction.
const PULSE_TRANSITION: Duration = Duration::from_millis(1200);

/// The light PUT body that puts one snapshot back: an off light is only
/// turned off, a ct light restores its mirek, an xy light both coordinates.
pub fn restore_body(state: &LightState) -> String {
    if !state.on {
        return serde_json::json!({"on": {"on": false}}).to_string();
    }
    let mut body = serde_json::json!({
        "on": {"on": true},
        "dimming": {"brightness": state.brightness},
    });
    if state.mode == "ct" {
        body["color_temperature"] =
            serde_json::json!({"mirek": state.v1.parse::<u64>().unwrap_or_default()});
    } else {
        body["color"] = serde_json::json!({"xy": {"x": number(&state.v1), "y": number(&state.v2)}});
    }
    body.to_string()
}

/// The bridge seam: authenticated GETs and PUTs against the CLIP paths.
pub trait Bridge {
    fn get(&self, path: &str) -> Option<String>;
    fn put(&self, path: &str, body: &str) -> bool;
}

/// The clock seam, so tests never sleep.
pub trait Sleeper {
    fn sleep(&self, duration: Duration);
}

/// The pulse: snapshot, four steps ending low, restore.
pub struct HuePulse<B: Bridge, S: Sleeper> {
    pub bridge: B,
    pub sleeper: S,
    pub rooms: Vec<String>,
}

impl<B: Bridge, S: Sleeper> HuePulse<B, S> {
    pub fn run(&self, exit_code: &str) {
        let Some(rooms_json) = self.bridge.get("room") else {
            return;
        };
        let grouped = grouped_light_ids(&rooms_json, &self.rooms);
        if grouped.is_empty() {
            return;
        }
        let Some(lights_json) = self.bridge.get("light") else {
            return;
        };
        let snapshot = light_snapshot(&lights_json, &room_device_ids(&rooms_json, &self.rooms));
        if snapshot.is_empty() {
            return;
        }

        // Two heartbeat cycles ENDING LOW, so the restore is a gentle step up
        // rather than a drop from peak.
        let color = crate::pulse::pulse_color(exit_code);
        let peak = color.peak_brightness.to_string();
        for (step, brightness) in [peak.as_str(), "20", peak.as_str(), "20"]
            .into_iter()
            .enumerate()
        {
            let body = pulse_body(color.x, color.y, brightness);
            for (room, id) in grouped.iter().enumerate() {
                let reached = self.bridge.put(&format!("grouped_light/{id}"), &body);
                // The very first PUT gates everything: a bridge unreachable
                // here leaves the lights untouched, so writing a restore over
                // state we never actually changed would be the real damage.
                if !reached && step == 0 && room == 0 {
                    return;
                }
            }
            self.sleeper.sleep(PULSE_TRANSITION);
        }

        for state in &snapshot {
            self.bridge
                .put(&format!("light/{}", state.id), &restore_body(state));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Bridge, DEFAULT_ROOMS, HuePulse, LightState, Sleeper, grouped_light_ids, hue_settings,
        light_snapshot, pulse_body, restore_body, room_device_ids,
    };
    use std::cell::RefCell;
    use std::time::Duration;

    const ROOMS_JSON: &str = r#"{"data":[
      {"id":"room-1","type":"room","metadata":{"name":"3F - Studio"},
       "children":[{"rid":"dev-a","rtype":"device"},{"rid":"dev-b","rtype":"device"}],
       "services":[{"rid":"grp-1","rtype":"grouped_light"}]},
      {"id":"room-2","type":"room","metadata":{"name":"2F - Kitchen"},
       "children":[{"rid":"dev-c","rtype":"device"}],
       "services":[{"rid":"grp-2","rtype":"grouped_light"}]}
    ]}"#;

    const LIGHTS_JSON: &str = r#"{"data":[
      {"id":"light-ct","type":"light","owner":{"rid":"dev-a","rtype":"device"},
       "on":{"on":true},"dimming":{"brightness":73.5},
       "color_temperature":{"mirek":366,"mirek_valid":true},
       "color":{"xy":{"x":0.4573,"y":0.41}}},
      {"id":"light-xy","type":"light","owner":{"rid":"dev-b","rtype":"device"},
       "on":{"on":true},"dimming":{"brightness":100},
       "color_temperature":{"mirek":null,"mirek_valid":false},
       "color":{"xy":{"x":0.2731,"y":0.6549}}},
      {"id":"light-off","type":"light","owner":{"rid":"dev-c","rtype":"device"},
       "on":{"on":false},"dimming":{"brightness":50},
       "color_temperature":{"mirek":300,"mirek_valid":true},
       "color":{"xy":{"x":0.3,"y":0.3}}},
      {"id":"light-elsewhere","type":"light","owner":{"rid":"dev-z","rtype":"device"},
       "on":{"on":true},"dimming":{"brightness":10},
       "color_temperature":{"mirek":200,"mirek_valid":true},
       "color":{"xy":{"x":0.1,"y":0.1}}}
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
    fn each_wanted_room_yields_its_grouped_light_and_a_renamed_room_is_skipped() {
        assert_eq!(
            grouped_light_ids(
                ROOMS_JSON,
                &wanted(&["3F - Studio", "Gone Room", "2F - Kitchen"])
            ),
            vec!["grp-1", "grp-2"]
        );
        assert!(grouped_light_ids(ROOMS_JSON, &wanted(&["Gone Room"])).is_empty());
        assert!(grouped_light_ids("not json", &wanted(&["3F - Studio"])).is_empty());
    }

    #[test]
    fn the_wanted_rooms_devices_are_collected_for_the_light_mapping() {
        assert_eq!(
            room_device_ids(ROOMS_JSON, &wanted(&["3F - Studio"])),
            vec!["dev-a", "dev-b"]
        );
    }

    #[test]
    fn the_snapshot_keeps_exactly_the_wanted_rooms_lights_with_their_modes() {
        let snapshot = light_snapshot(LIGHTS_JSON, &wanted(&["dev-a", "dev-b", "dev-c"]));
        assert_eq!(snapshot.len(), 3, "the elsewhere light is not ours");
        assert_eq!(
            snapshot[0],
            LightState {
                id: "light-ct".to_string(),
                on: true,
                brightness: 73.5,
                mode: "ct".to_string(),
                v1: "366".to_string(),
                v2: String::new(),
            }
        );
        assert_eq!(snapshot[1].mode, "xy");
        assert_eq!(snapshot[1].v1, "0.2731");
        assert_eq!(snapshot[1].v2, "0.6549");
        assert!(!snapshot[2].on);
    }

    // --- the bodies ---------------------------------------------------------

    #[test]
    fn a_pulse_step_turns_on_colors_dims_and_ramps_over_1200ms() {
        let parsed: serde_json::Value =
            serde_json::from_str(&pulse_body("0.2731", "0.6549", "70")).unwrap();
        assert_eq!(parsed["on"]["on"], true);
        assert_eq!(parsed["color"]["xy"]["x"], 0.2731);
        assert_eq!(parsed["color"]["xy"]["y"], 0.6549);
        assert_eq!(parsed["dimming"]["brightness"], 70.0);
        assert_eq!(parsed["dynamics"]["duration"], 1200);
    }

    #[test]
    fn an_off_light_is_restored_off_and_nothing_else() {
        let body = restore_body(&LightState {
            id: "light-off".to_string(),
            on: false,
            brightness: 50.0,
            mode: "ct".to_string(),
            v1: "300".to_string(),
            v2: String::new(),
        });
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["on"]["on"], false);
        assert_eq!(
            parsed.as_object().unwrap().len(),
            1,
            "off is the whole restore"
        );
    }

    #[test]
    fn a_ct_light_restores_its_mirek_and_an_xy_light_both_coordinates() {
        let ct: serde_json::Value = serde_json::from_str(&restore_body(&LightState {
            id: "l".to_string(),
            on: true,
            brightness: 73.5,
            mode: "ct".to_string(),
            v1: "366".to_string(),
            v2: String::new(),
        }))
        .unwrap();
        assert_eq!(ct["on"]["on"], true);
        assert_eq!(ct["dimming"]["brightness"], 73.5);
        assert_eq!(ct["color_temperature"]["mirek"], 366);
        let xy: serde_json::Value = serde_json::from_str(&restore_body(&LightState {
            id: "l".to_string(),
            on: true,
            brightness: 100.0,
            mode: "xy".to_string(),
            v1: "0.2731".to_string(),
            v2: "0.6549".to_string(),
        }))
        .unwrap();
        assert_eq!(xy["color"]["xy"]["x"], 0.2731);
        assert_eq!(xy["color"]["xy"]["y"], 0.6549);
    }

    // --- the sequence -------------------------------------------------------

    struct ScriptedBridge {
        rooms: &'static str,
        lights: &'static str,
        first_put_fails: bool,
        gets: RefCell<Vec<String>>,
        puts: RefCell<Vec<(String, String)>>,
    }

    impl Bridge for ScriptedBridge {
        fn get(&self, path: &str) -> Option<String> {
            self.gets.borrow_mut().push(path.to_string());
            if path.contains("room") {
                Some(self.rooms.to_string())
            } else {
                Some(self.lights.to_string())
            }
        }
        fn put(&self, path: &str, body: &str) -> bool {
            let first = self.puts.borrow().is_empty();
            self.puts
                .borrow_mut()
                .push((path.to_string(), body.to_string()));
            !(first && self.first_put_fails)
        }
    }

    struct CountingSleeper {
        naps: RefCell<Vec<Duration>>,
    }
    impl Sleeper for CountingSleeper {
        fn sleep(&self, duration: Duration) {
            self.naps.borrow_mut().push(duration);
        }
    }

    fn pulse(first_put_fails: bool) -> HuePulse<ScriptedBridge, CountingSleeper> {
        HuePulse {
            bridge: ScriptedBridge {
                rooms: ROOMS_JSON,
                lights: LIGHTS_JSON,
                first_put_fails,
                gets: RefCell::new(Vec::new()),
                puts: RefCell::new(Vec::new()),
            },
            sleeper: CountingSleeper {
                naps: RefCell::new(Vec::new()),
            },
            rooms: wanted(&["3F - Studio", "2F - Kitchen"]),
        }
    }

    #[test]
    fn a_green_pulse_hits_both_grouped_lights_four_times_then_restores_each_light() {
        let hue = pulse(false);
        hue.run("0");
        let puts = hue.bridge.puts.borrow();
        // 4 steps x 2 grouped lights, then 3 light restores.
        assert_eq!(puts.len(), 11);
        assert!(puts[0].0.contains("grouped_light/grp-1"));
        assert!(puts[1].0.contains("grouped_light/grp-2"));
        assert!(puts[8].0.contains("light/light-ct"));
        let naps = hue.sleeper.naps.borrow();
        assert_eq!(naps.as_slice(), &[Duration::from_millis(1200); 4]);
    }

    #[test]
    fn the_pulse_ends_low_so_the_restore_steps_up_gently() {
        let hue = pulse(false);
        hue.run("0");
        let puts = hue.bridge.puts.borrow();
        let step = |index: usize| -> f64 {
            let parsed: serde_json::Value = serde_json::from_str(&puts[index].1).unwrap();
            parsed["dimming"]["brightness"].as_f64().unwrap()
        };
        let peak = step(0);
        assert!(peak > 20.0);
        assert_eq!(step(2), 20.0);
        assert_eq!(step(4), peak);
        assert_eq!(step(6), 20.0);
    }

    #[test]
    fn a_failing_exit_code_pulses_the_red_corner_and_success_the_green() {
        let green = pulse(false);
        green.run("0");
        let red = pulse(false);
        red.run("9");
        let body = |hue: &HuePulse<ScriptedBridge, CountingSleeper>| -> serde_json::Value {
            serde_json::from_str(&hue.bridge.puts.borrow()[0].1).unwrap()
        };
        let green_x = body(&green)["color"]["xy"]["x"].as_f64().unwrap();
        let red_x = body(&red)["color"]["xy"]["x"].as_f64().unwrap();
        assert!(red_x > green_x, "red sits at the warm corner of the gamut");
    }

    #[test]
    fn an_unreachable_bridge_on_the_first_step_bails_without_touching_more() {
        let hue = pulse(true);
        hue.run("0");
        let puts = hue.bridge.puts.borrow();
        // The failed first grouped PUT is the last call: no second room, no
        // second step, and above all no restore writes over unknown state.
        assert_eq!(puts.len(), 1);
        assert!(hue.sleeper.naps.borrow().is_empty());
    }

    #[test]
    fn no_matching_rooms_or_no_lights_is_a_silent_no_op() {
        let hue = HuePulse {
            bridge: ScriptedBridge {
                rooms: r#"{"data":[]}"#,
                lights: r#"{"data":[]}"#,
                first_put_fails: false,
                gets: RefCell::new(Vec::new()),
                puts: RefCell::new(Vec::new()),
            },
            sleeper: CountingSleeper {
                naps: RefCell::new(Vec::new()),
            },
            rooms: wanted(&["3F - Studio"]),
        };
        hue.run("0");
        assert!(hue.bridge.puts.borrow().is_empty());
    }
}
