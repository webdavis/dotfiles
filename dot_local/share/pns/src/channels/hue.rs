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

/// One pulse-able room: its grouped_light and the snapshots of ITS lights,
/// paired, because pulsing a room whose lights cannot be restored leaves
/// them stuck in the transient color. A room missing either half is
/// excluded whole.
#[derive(Debug, PartialEq)]
pub struct PulseRoom {
    pub grouped_id: String,
    pub lights: Vec<LightState>,
}

/// The wanted rooms as pulse units, in wanted order: a renamed room, a room
/// without a grouped_light, and a room contributing no restorable light are
/// each skipped, never fatal and never pulsed.
pub fn pulse_rooms(rooms_json: &str, lights_json: &str, wanted: &[String]) -> Vec<PulseRoom> {
    wanted
        .iter()
        .filter_map(|name| {
            let one = std::slice::from_ref(name);
            Some(PulseRoom {
                grouped_id: room_rids(rooms_json, one, "services", "grouped_light")
                    .into_iter()
                    .next()?,
                lights: light_snapshot(
                    lights_json,
                    &room_rids(rooms_json, one, "children", "device"),
                ),
            })
            .filter(|room| !room.lights.is_empty())
        })
        .collect()
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
            // A reading the restore needs is REQUIRED, never defaulted:
            // inventing off for a missing on, or zero for a missing
            // coordinate, corrupts a light this pulse never read correctly.
            let rendered = |path: &str| {
                light
                    .pointer(path)
                    .filter(|value| !value.is_null())
                    .map(|value| value.to_string())
            };
            let ct = light.pointer("/color_temperature/mirek_valid")
                == Some(&serde_json::Value::Bool(true));
            let (v1, v2) = if ct {
                (rendered("/color_temperature/mirek")?, String::new())
            } else {
                (rendered("/color/xy/x")?, rendered("/color/xy/y")?)
            };
            Some(LightState {
                id: light.get("id")?.as_str()?.to_string(),
                on: light.pointer("/on/on")?.as_bool()?,
                // Brightness alone keeps the bash jq default: an absent
                // dimming meant 100, not a light restored to darkness.
                brightness: light
                    .pointer("/dimming/brightness")
                    .and_then(|brightness| brightness.as_f64())
                    .unwrap_or(100.0),
                mode: if ct { "ct" } else { "xy" }.to_string(),
                v1,
                v2,
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

/// How long ONE ramp takes, the value the bridge is handed in `dynamics`. The
/// pace between steps is [`STEP_SETTLE`], which is deliberately longer.
const PULSE_TRANSITION: Duration = Duration::from_millis(1200);

/// How long we wait after a step's write before making the next one: the ramp
/// plus 250ms of bridge breathing room.
///
/// The gap is NOT the ramp finishing, which `PULSE_TRANSITION` already covers.
/// Drill D2 2026-08-12: with the steps paced at exactly the ramp length the
/// fourth write never rendered, and the pulse read peak, dim, peak, restore.
/// The SUSPICION, unproven, is bridge-side rate limiting on grouped_light PUTs
/// landing on exact 1200ms boundaries; the bash channel's per-step process
/// spawns would have given it ragged slack for free, which would explain why
/// it never showed this, though that channel is gone and the comparison was
/// never measured. Either way a dropped write is SILENT here, because only the
/// FIRST write's refusal stops the pulse. Empirical, like the hold below: the
/// 250ms buys slack, it does not explain the bridge.
const STEP_SETTLE: Duration = Duration::from_millis(1450);

/// The last dim is HELD before the restore ramps the lights back up. Live
/// finding 2026-08-11: the restore fired the instant the fourth ramp's sleep
/// returned and overrode the final dim before the bridge rendered it, so the
/// pulse read as three phases, not four.
///
/// The VALUE is empirical padding for bridge-side render latency and nothing
/// more principled than that: the ramp it follows had already been given its
/// full transition time, so the gap it covers is unexplained and unmeasured.
/// Drill D2 2026-08-12 retuned it from half a `PULSE_TRANSITION` to a full
/// one, because at 600ms the last dim still never rendered. Retune again if a
/// drill still shows three phases.
const FINAL_DIM_HOLD: Duration = Duration::from_millis(1200);

/// The light PUT body that puts one snapshot back: an off light is only
/// turned off, a ct light restores its mirek, an xy light both coordinates.
pub fn restore_body(state: &LightState) -> String {
    if !state.on {
        return serde_json::json!({
            "on": {"on": false},
            "dynamics": {"duration": RESTORE_TRANSITION_MS},
        })
        .to_string();
    }
    let mut body = serde_json::json!({
        "on": {"on": true},
        "dimming": {"brightness": state.brightness},
        "dynamics": {"duration": RESTORE_TRANSITION_MS},
    });
    if state.mode == "ct" {
        body["color_temperature"] =
            serde_json::json!({"mirek": state.v1.parse::<u64>().unwrap_or_default()});
    } else {
        body["color"] = serde_json::json!({"xy": {"x": number(&state.v1), "y": number(&state.v2)}});
    }
    body.to_string()
}

/// The restore ramp the R1 pulse module pinned for both restore arms, in
/// milliseconds for the CLIP body.
pub const RESTORE_TRANSITION_MS: u64 = 500;

/// Whether one CLIP write actually applied: status success is not enough,
/// because the bridge answers 200 with a NONEMPTY errors array for
/// application failures, and the first-PUT bail must see those too.
pub fn put_succeeded(status_ok: bool, body: &str) -> bool {
    status_ok
        && !serde_json::from_str::<serde_json::Value>(body)
            .ok()
            .and_then(|body| Some(!body.get("errors")?.as_array()?.is_empty()))
            .unwrap_or(false)
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

/// The single-pulse lock, on the SAME path the bash channel locks so the
/// two cannot interleave during the repoint window. None while another
/// pulse holds it; the kernel releases it on any exit.
pub fn acquire_pulse_lock(state_dir: &std::path::Path) -> Option<std::fs::File> {
    std::fs::create_dir_all(state_dir).ok()?;
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(state_dir.join("hue-pulse.lockf"))
        .ok()?;
    lock.try_lock().ok()?;
    Some(lock)
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
        let Some(lights_json) = self.bridge.get("light") else {
            return;
        };
        let rooms = pulse_rooms(&rooms_json, &lights_json, &self.rooms);
        if rooms.is_empty() {
            return;
        }

        let color = crate::pulse::pulse_color(exit_code);
        let (x, y, peak) = (color.x, color.y, color.peak_brightness);
        let steps = [
            peak.to_string(),
            "20".to_string(),
            peak.to_string(),
            "20".to_string(),
        ];
        let mut first = true;
        for brightness in steps {
            for room in &rooms {
                let applied = self.bridge.put(
                    &format!("grouped_light/{}", room.grouped_id),
                    &pulse_body(x, y, &brightness),
                );
                // The FIRST write gates the whole pulse: an unreachable or
                // refusing bridge must not leave restores written over state
                // this run never successfully changed.
                if first && !applied {
                    return;
                }
                first = false;
            }
            self.sleeper.sleep(STEP_SETTLE);
        }
        self.sleeper.sleep(FINAL_DIM_HOLD);

        // Every light is restored independently: one failing write must not
        // starve the lights behind it in the transient color.
        for room in &rooms {
            for light in &room.lights {
                self.bridge
                    .put(&format!("light/{}", light.id), &restore_body(light));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Bridge, DEFAULT_ROOMS, HuePulse, LightState, RESTORE_TRANSITION_MS, Sleeper,
        acquire_pulse_lock, hue_enabled, hue_settings, light_snapshot, pulse_body, pulse_rooms,
        put_succeeded, restore_body,
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
    fn each_wanted_room_pairs_its_group_with_its_own_lights_in_wanted_order() {
        let rooms = pulse_rooms(
            ROOMS_JSON,
            LIGHTS_JSON,
            &wanted(&["2F - Kitchen", "3F - Studio"]),
        );
        assert_eq!(rooms.len(), 2);
        assert_eq!(rooms[0].grouped_id, "grp-2");
        assert_eq!(
            rooms[0]
                .lights
                .iter()
                .map(|l| l.id.as_str())
                .collect::<Vec<_>>(),
            vec!["light-off"]
        );
        assert_eq!(rooms[1].grouped_id, "grp-1");
        assert_eq!(
            rooms[1]
                .lights
                .iter()
                .map(|l| l.id.as_str())
                .collect::<Vec<_>>(),
            vec!["light-ct", "light-xy"]
        );
    }

    #[test]
    fn a_room_whose_lights_cannot_be_restored_is_never_pulsed() {
        // Pulsing a room whose lights are not in the snapshot would leave
        // them stuck in the transient color forever.
        const NO_LIGHTS: &str = r#"{"data":[
          {"id":"room-1","type":"room","metadata":{"name":"Empty Room"},
           "children":[],"services":[{"rid":"grp-1","rtype":"grouped_light"}]}
        ]}"#;
        assert!(pulse_rooms(NO_LIGHTS, LIGHTS_JSON, &wanted(&["Empty Room"])).is_empty());
    }

    #[test]
    fn a_room_without_a_grouped_light_is_skipped_whole() {
        const NO_GROUP: &str = r#"{"data":[
          {"id":"room-1","type":"room","metadata":{"name":"Groupless"},
           "children":[{"rid":"dev-a","rtype":"device"}],"services":[]}
        ]}"#;
        assert!(pulse_rooms(NO_GROUP, LIGHTS_JSON, &wanted(&["Groupless"])).is_empty());
    }

    #[test]
    fn a_renamed_room_is_skipped_and_unparseable_json_is_empty() {
        assert!(pulse_rooms(ROOMS_JSON, LIGHTS_JSON, &wanted(&["Gone Room"])).is_empty());
        assert!(pulse_rooms("not json", LIGHTS_JSON, &wanted(&["3F - Studio"])).is_empty());
    }

    #[test]
    fn a_light_missing_a_reading_the_restore_needs_is_skipped_never_invented() {
        // Inventing off for a missing on, or zero for a missing coordinate,
        // would corrupt a light this pulse never correctly touched.
        const PARTIAL: &str = r#"{"data":[
          {"id":"no-on","type":"light","owner":{"rid":"dev-a","rtype":"device"},
           "dimming":{"brightness":50},"color_temperature":{"mirek":300,"mirek_valid":true}},
          {"id":"no-xy","type":"light","owner":{"rid":"dev-a","rtype":"device"},
           "on":{"on":true},"dimming":{"brightness":50},
           "color_temperature":{"mirek_valid":false},"color":{}},
          {"id":"no-mirek","type":"light","owner":{"rid":"dev-a","rtype":"device"},
           "on":{"on":true},"dimming":{"brightness":50},
           "color_temperature":{"mirek_valid":true}},
          {"id":"whole","type":"light","owner":{"rid":"dev-a","rtype":"device"},
           "on":{"on":true},"color_temperature":{"mirek":300,"mirek_valid":true},
           "color":{"xy":{"x":0.3,"y":0.3}}}
        ]}"#;
        let snapshot = light_snapshot(PARTIAL, &wanted(&["dev-a"]));
        assert_eq!(
            snapshot.iter().map(|l| l.id.as_str()).collect::<Vec<_>>(),
            vec!["whole"],
            "only the light with every reading the restore needs"
        );
        assert_eq!(
            snapshot[0].brightness, 100.0,
            "absent dimming keeps the bash default"
        );
    }

    #[test]
    fn a_two_hundred_carrying_errors_is_not_an_applied_write() {
        // The bridge answers 200 with a nonempty errors array for
        // application failures; the first-PUT bail must see those.
        assert!(put_succeeded(true, r#"{"errors":[],"data":[{"rid":"x"}]}"#));
        assert!(put_succeeded(true, r#"{"data":[]}"#));
        assert!(!put_succeeded(
            true,
            r#"{"errors":[{"description":"device is not responding"}],"data":[]}"#
        ));
        assert!(!put_succeeded(false, r#"{"errors":[]}"#));
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

    #[test]
    fn a_second_pulse_is_skipped_while_the_first_holds_the_lock() {
        let dir = std::env::temp_dir().join(format!("pns-hue-lock-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let held = acquire_pulse_lock(&dir).expect("the first pulse takes the lock");
        assert!(
            acquire_pulse_lock(&dir).is_none(),
            "a concurrent pulse is skipped, never interleaved"
        );
        drop(held);
        assert!(
            acquire_pulse_lock(&dir).is_some(),
            "the lock is released with the holder"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_lock_path_is_the_one_the_bash_channel_takes() {
        // Different paths would let the native and bash pulses interleave
        // during the repoint window.
        let dir = std::env::temp_dir().join(format!("pns-hue-lockpath-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let _held = acquire_pulse_lock(&dir);
        assert!(dir.join("hue-pulse.lockf").exists());
        std::fs::remove_dir_all(&dir).ok();
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
    fn an_off_light_is_restored_off_and_carries_no_brightness_or_color() {
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
        assert!(
            parsed.get("dimming").is_none(),
            "an off light restores no brightness"
        );
        assert!(
            parsed.get("color").is_none() && parsed.get("color_temperature").is_none(),
            "an off light restores no color"
        );
    }

    #[test]
    fn every_restore_carries_the_ramp_the_pulse_module_pinned() {
        for state in [
            LightState {
                id: "l".to_string(),
                on: false,
                brightness: 50.0,
                mode: "ct".to_string(),
                v1: "300".to_string(),
                v2: String::new(),
            },
            LightState {
                id: "l".to_string(),
                on: true,
                brightness: 50.0,
                mode: "ct".to_string(),
                v1: "300".to_string(),
                v2: String::new(),
            },
        ] {
            let parsed: serde_json::Value = serde_json::from_str(&restore_body(&state)).unwrap();
            assert_eq!(
                parsed["dynamics"]["duration"], RESTORE_TRANSITION_MS,
                "both restore arms ramp, as the R1 decision spells"
            );
        }
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
        /// A path substring whose PUT fails, for the restore-independence pin.
        fail_put_containing: Option<&'static str>,
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
            if self
                .fail_put_containing
                .is_some_and(|needle| path.contains(needle))
            {
                return false;
            }
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
                fail_put_containing: None,
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
        assert_eq!(
            naps.as_slice(),
            &[
                Duration::from_millis(1450),
                Duration::from_millis(1450),
                Duration::from_millis(1450),
                Duration::from_millis(1450),
                // The literals, not the constants: comparing a pace against
                // itself would pass for any value, including one too short
                // for the bridge to render or accept.
                Duration::from_millis(1200),
            ],
            "four settles longer than the ramp, then the hold that lets the last dim render"
        );
    }

    /// One shared, ordered log across bridge and sleeper: the hold's PLACE
    /// in the sequence is the behavior under test, and the two separate
    /// recorders above cannot see cross-seam order.
    struct SequencedBridge {
        log: std::rc::Rc<RefCell<Vec<String>>>,
    }
    impl Bridge for SequencedBridge {
        fn get(&self, path: &str) -> Option<String> {
            Some(match path {
                "room" => ROOMS_JSON.to_string(),
                _ => LIGHTS_JSON.to_string(),
            })
        }
        fn put(&self, path: &str, _body: &str) -> bool {
            self.log.borrow_mut().push(format!("put {path}"));
            true
        }
    }
    struct SequencedSleeper {
        log: std::rc::Rc<RefCell<Vec<String>>>,
    }
    impl Sleeper for SequencedSleeper {
        fn sleep(&self, duration: Duration) {
            self.log
                .borrow_mut()
                .push(format!("nap {}", duration.as_millis()));
        }
    }

    #[test]
    fn the_final_dim_is_held_before_any_restore_write_so_it_renders() {
        // Live finding 2026-08-11: the restore fired the instant the fourth
        // ramp's sleep returned, and the physical bridge never rendered the
        // second dim. The contract is peak, dim, peak, dim, EACH phase
        // visible, then restore: five naps, and the fifth sits between the
        // last grouped-light write and the first light restore.
        let log = std::rc::Rc::new(RefCell::new(Vec::new()));
        let hue = HuePulse {
            bridge: SequencedBridge {
                log: std::rc::Rc::clone(&log),
            },
            sleeper: SequencedSleeper {
                log: std::rc::Rc::clone(&log),
            },
            rooms: wanted(&["3F - Studio", "2F - Kitchen"]),
        };
        hue.run("1");
        let log = log.borrow();
        let naps: Vec<usize> = (0..log.len())
            .filter(|index| log[*index].starts_with("nap"))
            .collect();
        assert_eq!(
            naps.len(),
            5,
            "four ramps plus the final hold, got: {log:?}"
        );
        let first_restore = log
            .iter()
            .position(|entry| entry.starts_with("put light/"))
            .expect("the restores must still run");
        assert!(
            naps[4] < first_restore,
            "the hold must precede every restore write: {log:?}"
        );
        let hold = log[naps[4]]
            .strip_prefix("nap ")
            .unwrap()
            .parse::<u64>()
            .unwrap();
        assert_eq!(
            hold, 1200,
            "the hold is the full ramp length: D2 showed 600ms still lost the last dim"
        );
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
    fn one_failing_restore_never_starves_the_lights_behind_it() {
        // A flaky write would otherwise leave every later light stuck in the
        // transient pulse color at 20 percent.
        let hue = HuePulse {
            bridge: ScriptedBridge {
                rooms: ROOMS_JSON,
                lights: LIGHTS_JSON,
                first_put_fails: false,
                fail_put_containing: Some("light/light-ct"),
                gets: RefCell::new(Vec::new()),
                puts: RefCell::new(Vec::new()),
            },
            sleeper: CountingSleeper {
                naps: RefCell::new(Vec::new()),
            },
            rooms: wanted(&["3F - Studio", "2F - Kitchen"]),
        };
        hue.run("0");
        let puts = hue.bridge.puts.borrow();
        let restored: Vec<&str> = puts
            .iter()
            .filter(|(path, _)| path.starts_with("light/"))
            .map(|(path, _)| path.as_str())
            .collect();
        assert_eq!(
            restored,
            vec!["light/light-ct", "light/light-xy", "light/light-off"],
            "every light is attempted, whatever one write did"
        );
    }

    #[test]
    fn no_matching_rooms_or_no_lights_is_a_silent_no_op() {
        let hue = HuePulse {
            bridge: ScriptedBridge {
                rooms: r#"{"data":[]}"#,
                lights: r#"{"data":[]}"#,
                first_put_fails: false,
                fail_put_containing: None,
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
