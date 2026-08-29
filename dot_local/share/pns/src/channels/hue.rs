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

/// The hours the lights stay dark, in minutes since local midnight.
#[derive(Debug, PartialEq)]
pub struct QuietWindow {
    start: u16,
    end: u16,
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

/// Whether the lights are inside the window at a given minute of the local
/// day.
pub fn quiet_now(window: Option<&QuietWindow>, minutes_now: Option<u16>) -> bool {
    // NO WINDOW IS NEVER QUIET, whatever the clock says: an operator who
    // configured no quiet hours keeps the pulse an unreadable clock would
    // otherwise cost them.
    let Some(window) = window else {
        return false;
    };
    // A CONFIGURED window and no clock FAILS CLOSED, the direction the pulse
    // already takes on an unreadable reading: a missed pulse costs nothing and
    // a flash at 3am is what the window was set to prevent.
    let Some(now) = minutes_now else {
        return true;
    };
    if window.start > window.end {
        // A window that wraps midnight is the two ends of the day joined, so
        // the halves are an OR: past its start tonight, or before its end
        // tomorrow.
        return now >= window.start || now < window.end;
    }
    now >= window.start && now < window.end
}

/// The refusal, in the shape the config layer already refuses a setting by
/// name: what was written, and what it cost.
fn quiet_hours_refusal(offender: &str) -> String {
    format!("pns: config error (hue.quiet_hours is {offender}, not a HH:MM-HH:MM window); no pulse")
}

/// `HH:MM-HH:MM` and nothing else.
fn parse_window(text: &str) -> Option<QuietWindow> {
    let (start, end) = text.split_once('-')?;
    Some(QuietWindow {
        start: minute_of_day(start)?,
        end: minute_of_day(end)?,
    })
}

/// `HH:MM` as minutes since midnight. Two digits each, and in range: an hour
/// of 24 or a minute of 60 names no time of day.
fn minute_of_day(clock: &str) -> Option<u16> {
    let (hours, minutes) = clock.split_once(':')?;
    let (hours, minutes) = (two_digits(hours)?, two_digits(minutes)?);
    (hours < 24 && minutes < 60).then_some(hours * 60 + minutes)
}

/// Exactly two ASCII digits, so a sign, a space or a lone digit is not a
/// clock reading that happens to parse.
fn two_digits(text: &str) -> Option<u16> {
    if text.len() != 2 || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
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
        Bridge, DEFAULT_ROOMS, HuePulse, QuietWindow, grouped_light_ids_for_rooms, hue_settings,
        quiet_now, quiet_window,
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

    // --- the quiet window ---------------------------------------------------

    #[test]
    fn a_table_that_names_no_quiet_hours_has_no_window() {
        assert_eq!(
            quiet_window(&table("bridge = \"b\"\nkey = \"k\"")),
            Ok(None),
            "an operator who never asked to be quieted keeps today's behavior"
        );
    }

    #[test]
    fn a_window_parses_to_minutes_since_local_midnight() {
        assert_eq!(
            quiet_window(&table("quiet_hours = \"22:00-07:00\"")),
            Ok(Some(QuietWindow {
                start: 1320,
                end: 420
            })),
            "22:00 is 1320 minutes in and 07:00 is 420"
        );
    }

    #[test]
    fn a_quiet_hours_that_is_not_two_clock_readings_is_refused_by_name() {
        for stated in [
            "22:00",
            "24:00-07:00",
            "22:60-07:00",
            "10pm-7am",
            "2:00-07:00",
            "22:00-07:00 ",
            "   ",
        ] {
            let refusal = quiet_window(&table(&format!("quiet_hours = \"{stated}\"")))
                .expect_err("a window this shape names no hours");
            assert!(
                refusal.contains("hue.quiet_hours") && refusal.contains(stated),
                "the refusal names the key and echoes what was written: {refusal}"
            );
        }
    }

    #[test]
    fn a_quiet_hours_of_the_wrong_type_is_refused_by_name_and_by_type() {
        for (stated, kind) in [("2200", "integer"), ("true", "boolean"), ("[]", "array")] {
            let refusal = quiet_window(&table(&format!("quiet_hours = {stated}")))
                .expect_err("a window that is not a string names no hours");
            assert!(
                refusal.contains("hue.quiet_hours") && refusal.contains(kind),
                "the refusal names the key and what was written instead: {refusal}"
            );
        }
    }

    #[test]
    fn a_blanked_quiet_hours_is_no_window_rather_than_a_refusal() {
        assert_eq!(
            quiet_window(&table("quiet_hours = \"\"")),
            Ok(None),
            "blanking a value plainly means none, the way an empty bridge or key does"
        );
    }

    /// 22:00 to 23:00, the plainest same-day window.
    const EVENING: QuietWindow = QuietWindow {
        start: 1320,
        end: 1380,
    };

    #[test]
    fn a_same_day_window_is_quiet_from_its_start_and_loud_again_at_its_end() {
        assert!(
            !quiet_now(Some(&EVENING), Some(1319)),
            "the minute before the window is loud"
        );
        assert!(
            quiet_now(Some(&EVENING), Some(1320)),
            "the start is inside the window"
        );
        assert!(
            quiet_now(Some(&EVENING), Some(1379)),
            "and so is the last minute before its end"
        );
        assert!(
            !quiet_now(Some(&EVENING), Some(1380)),
            "the end is loud on the dot, so two adjacent windows cannot overlap"
        );
    }

    #[test]
    fn a_window_whose_start_is_after_its_end_is_quiet_on_both_sides_of_midnight() {
        // 22:00-07:00, the window the template documents.
        let overnight = QuietWindow {
            start: 1320,
            end: 420,
        };
        for (minute, quiet, moment) in [
            (1319, false, "21:59, before it opens"),
            (1320, true, "22:00, the start"),
            (1439, true, "23:59, the last minute of the day"),
            (0, true, "00:00, the first minute of the next one"),
            (419, true, "06:59, still inside"),
            (420, false, "07:00, the end"),
            (720, false, "noon, nowhere near it"),
        ] {
            assert_eq!(
                quiet_now(Some(&overnight), Some(minute)),
                quiet,
                "{moment} is on the wrong side of a window that wraps"
            );
        }
    }

    #[test]
    fn a_window_whose_start_equals_its_end_is_never_quiet() {
        // An empty half-open range, and deliberately not a special case: the
        // all-day mute already exists as `enabled = false`. Every minute of
        // the day is checked, because "never" is the whole claim.
        let empty = QuietWindow {
            start: 600,
            end: 600,
        };
        for minute in 0..1440 {
            assert!(
                !quiet_now(Some(&empty), Some(minute)),
                "minute {minute} fell inside a window that spans no time"
            );
        }
    }

    #[test]
    fn a_window_with_an_unreadable_clock_is_quiet() {
        assert!(
            quiet_now(Some(&EVENING), None),
            "an operator who asked for quiet hours and a clock that cannot say \
             whether it is one: a missed pulse costs nothing, a 3am flash does"
        );
    }

    #[test]
    fn no_window_and_an_unreadable_clock_mutes_nothing() {
        assert!(
            !quiet_now(None, None),
            "the fail-closed direction belongs to a window that was asked for; \
             without one there is no quiet hour to be inside"
        );
        assert!(!quiet_now(None, Some(180)), "nor at 3am");
    }
}
