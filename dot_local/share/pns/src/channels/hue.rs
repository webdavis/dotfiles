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

/// One resolved bridge target, and WHAT it is: a whole room addressed through
/// its grouped light, or one individual lamp.
///
/// The distinction is not cosmetic. A grouped light is one write for a whole
/// room and is all a room-shaped claim ever needs; a light is what any narrower
/// claim resolves to, and it is also the only kind whose prior state the bridge
/// will report back (a grouped_light GET carries no colour at all).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Fixture {
    Grouped(String),
    Light(String),
}

/// A claim in the config that answered no fixture, and who wrote it.
///
/// REPORTED, NEVER DROPPED. `grouped_light_ids_for_rooms` drops a missing room
/// in silence, which is survivable for a two-room pulse an operator can see
/// failing. A family map is large enough that a typo is a lamp which never
/// lights, and a silent drop makes it unfalsifiable.
///
/// `kind` is LAST so the sort stays by family then name: the doctor prints
/// these in order and that order must not depend on why each one is here.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Unresolved {
    pub family: String,
    pub name: String,
    pub kind: Missing,
}

/// WHY a claim answered no fixture, because the two are different problems and
/// an operator fixes them in different places.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Missing {
    /// No room and no lamp of that name in the bridge's listings. A spelling
    /// to fix, or a lamp to plug in.
    NotOnBridge,
    /// The name IS on the bridge and the claim still addressed no lamp: every
    /// member was excepted, or spoken for by a narrower claim, or is a device
    /// that owns no light. A map to fix, and telling an operator to go looking
    /// for a room that is sitting right there in the bridge would be a lie
    /// they act on.
    AddressedNothing,
}

/// One fixture two state-producing families both claimed, named with both of
/// them so the operator can break the tie with a `skip` list.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct StateConflict {
    pub place: String,
    pub fixture: Fixture,
    pub families: Vec<String>,
}

/// The whole map: what each family holds, what could not be found, and where
/// two families would fight over one lamp's state.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Resolution {
    pub families: std::collections::BTreeMap<String, Vec<Fixture>>,
    pub unresolved: Vec<Unresolved>,
    pub state_conflicts: Vec<StateConflict>,
}

impl Resolution {
    /// The fixtures a family may hold a STATE on: its own, minus every one a
    /// second state-producing family also claimed.
    ///
    /// A CONTESTED FIXTURE HOLDS NO STATE AT ALL rather than going to one of
    /// the claimants. Picking a winner here would be a rule nobody stated,
    /// applied to a lamp the operator is watching; a dark lamp beside a doctor
    /// line naming both families is the state they can act on.
    pub fn state_fixtures(&self, family: &str) -> Vec<Fixture> {
        self.families
            .get(family)
            .map(|fixtures| {
                fixtures
                    .iter()
                    .filter(|fixture| {
                        !self
                            .state_conflicts
                            .iter()
                            .any(|conflict| &conflict.fixture == *fixture)
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// Whether resolving these claims needs the bridge's LIGHT listing as well as
/// its room listing.
///
/// THE COST FENCE. A config naming only rooms resolves out of the one GET the
/// pulse has always made, so today's network cost does not move for an operator
/// who wrote the simplest map. Anything narrower (a light by name, or a room
/// with an exception carved out of it) needs the device join, and that join
/// lives in the light listing.
pub fn needs_light_listing(
    families: &std::collections::BTreeMap<String, crate::config::Family>,
) -> bool {
    families
        .values()
        .any(|family| !family.lights.is_empty() || !family.except.is_empty())
}

/// The families whose behaviours include a STATE, and therefore the only ones
/// that can contest a lamp.
///
/// `local` holds needs-you and `loop` holds breathing and glow. `github` is
/// PLUMBING ONLY today (operator ruling): it resolves lamps and nothing
/// produces its events, so it cannot contest one. That is why the studio map's
/// own `github` and `loop` claims on one lamp are not a conflict, and why the
/// day a GitHub feed lands is not the day a lamp starts lying.
pub const STATE_PRODUCING_FAMILIES: [&str; 2] = ["local", "loop"];

/// Every family name pns speaks: the two above, plus `github`, which resolves
/// lamps that nothing routes to yet (operator ruling: PLUMBING ONLY).
///
/// A NAME OUTSIDE THIS SET RESOLVES REAL LAMPS AND LIGHTS NONE OF THEM. The
/// config layer cannot judge it, since only the running crate knows which
/// families it produces, so the doctor is where the operator hears about it,
/// and this is the list it asks.
pub const KNOWN_FAMILIES: [&str; 3] = ["github", "local", "loop"];

/// One lamp in the bridge's light listing.
struct Lamp {
    id: String,
    name: String,
    /// The DEVICE that owns it, which is what a room lists as its child. The
    /// join runs through here and nowhere else.
    owner: String,
}

/// The bridge's light listing, in the order it answered.
fn lamps(lights_json: &str) -> Vec<Lamp> {
    data_entries(lights_json)
        .iter()
        .filter_map(|light| {
            Some(Lamp {
                id: light.get("id")?.as_str()?.to_string(),
                name: light.pointer("/metadata/name")?.as_str()?.to_string(),
                owner: light.pointer("/owner/rid")?.as_str()?.to_string(),
            })
        })
        .collect()
}

/// Names to fixtures: pure, total, and loud about what it could not find.
///
/// LIGHT CLAIMS ARE RESOLVED FIRST AND SUBTRACTED FROM EVERY ROOM, which is
/// what makes `except` and specific-beats-general the SAME line of code rather
/// than two rules that can disagree. A room keeps its grouped light only while
/// nothing has been taken out of it; the moment one lamp is spoken for, the
/// room is addressed lamp by lamp, because a group write would reach the lamp
/// that was carved out.
pub fn resolve(
    rooms_json: &str,
    lights_json: Option<&str>,
    families: &std::collections::BTreeMap<String, crate::config::Family>,
) -> Resolution {
    let lamps = lamps(lights_json.unwrap_or_default());
    let rooms = data_entries(rooms_json);
    let named = |name: &str| lamps.iter().find(|lamp| lamp.name == name);

    let mut resolution = Resolution::default();
    let mut names: std::collections::BTreeMap<Fixture, String> = std::collections::BTreeMap::new();
    let mut spoken_for: Vec<String> = Vec::new();

    // PASS ONE, every light claim, because pass two subtracts them.
    for (family, claims) in families {
        let mut held = Vec::new();
        for name in &claims.lights {
            match named(name) {
                Some(lamp) => {
                    let fixture = Fixture::Light(lamp.id.clone());
                    names.insert(fixture.clone(), lamp.name.clone());
                    spoken_for.push(lamp.id.clone());
                    held.push(fixture);
                }
                None => resolution.unresolved.push(Unresolved {
                    family: family.clone(),
                    name: name.clone(),
                    kind: Missing::NotOnBridge,
                }),
            }
        }
        resolution.families.insert(family.clone(), held);
    }

    // PASS TWO, the rooms, each one minus whatever is already spoken for.
    for (family, claims) in families {
        let mut excepted: Vec<String> = Vec::new();
        // AN EXCEPT THAT MISSED FAILS CLOSED for the whole family's rooms. The
        // operator asked for a lamp to be left out and named it wrong; the
        // group write would reach exactly the lamp they carved out, and it
        // would do it while the doctor reported the typo, so the report and the
        // write would say opposite things about the same lamp.
        let mut an_except_missed = false;
        for name in &claims.except {
            match named(name) {
                Some(lamp) => excepted.push(lamp.id.clone()),
                None => {
                    an_except_missed = true;
                    resolution.unresolved.push(Unresolved {
                        family: family.clone(),
                        name: name.clone(),
                        kind: Missing::NotOnBridge,
                    });
                }
            }
        }
        for room_name in &claims.rooms {
            let Some(room) = rooms.iter().find(|room| {
                room.pointer("/metadata/name")
                    .and_then(|name| name.as_str())
                    == Some(room_name.as_str())
            }) else {
                resolution.unresolved.push(Unresolved {
                    family: family.clone(),
                    name: room_name.clone(),
                    kind: Missing::NotOnBridge,
                });
                continue;
            };
            let children: Vec<&str> = room
                .get("children")
                .and_then(|children| children.as_array())
                .map(|children| {
                    children
                        .iter()
                        .filter_map(|child| child.get("rid")?.as_str())
                        .collect()
                })
                .unwrap_or_default();
            let members: Vec<&Lamp> = lamps
                .iter()
                .filter(|lamp| children.contains(&lamp.owner.as_str()))
                .collect();
            let kept: Vec<&&Lamp> = members
                .iter()
                .filter(|lamp| !excepted.contains(&lamp.id) && !spoken_for.contains(&lamp.id))
                .collect();

            let held = resolution.families.entry(family.clone()).or_default();
            // WHAT THIS ROOM ITSELF ADDED, which is the only honest way to say
            // whether this room addressed anything: the family's own list
            // carries every earlier claim, so asking whether IT is empty
            // silences every room after the first one that resolved.
            let before = held.len();
            let group = room
                .get("services")
                .and_then(|services| services.as_array())
                .map(|services| {
                    services
                        .iter()
                        .filter(|service| {
                            service.get("rtype").and_then(|kind| kind.as_str())
                                == Some("grouped_light")
                        })
                        .filter_map(|service| service.get("rid")?.as_str())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            match group.first() {
                // WHOLE AND UNTOUCHED is the only shape that keeps the group,
                // and a missed `except` is not untouched: it is a carve-out
                // whose target is unknown, so the room is addressed lamp by
                // lamp and the group write that would reach it is never made.
                Some(id) if !an_except_missed && kept.len() == members.len() => {
                    let fixture = Fixture::Grouped((*id).to_string());
                    names.insert(fixture.clone(), room_name.clone());
                    held.push(fixture);
                }
                _ => {
                    for lamp in kept {
                        let fixture = Fixture::Light(lamp.id.clone());
                        names.insert(fixture.clone(), lamp.name.clone());
                        held.push(fixture);
                    }
                }
            }
            // A ROOM THAT ADDRESSED NOTHING IS STILL REPORTED. A room with no
            // grouped light and no lamps this listing knows (the capture has
            // two of them), and a room carved down to nothing by its own
            // exceptions, would otherwise be the silent drop every refusal in
            // here exists to prevent. It is NOT a room the bridge is missing,
            // and it does not get that sentence.
            let addressed_nothing = held.len() == before;
            if addressed_nothing {
                resolution.unresolved.push(Unresolved {
                    family: family.clone(),
                    name: room_name.clone(),
                    kind: Missing::AddressedNothing,
                });
            }
        }
    }

    // A FAMILY HOLDS EACH FIXTURE ONCE, however many claims arrived at it. Two
    // spellings of one claim are one claim, and counting them twice made a
    // family its own second claimant in `contested`, which took the lamp's
    // state away over a conflict with nobody.
    for held in resolution.families.values_mut() {
        let mut seen = std::collections::BTreeSet::new();
        held.retain(|fixture| seen.insert(fixture.clone()));
    }

    // ORDERED, so the doctor prints the same lines in the same order for the
    // same bridge whatever order the listings arrived in.
    resolution.unresolved.sort();
    resolution.unresolved.dedup();
    resolution.state_conflicts = contested(&resolution.families, &names);
    resolution
}

/// The fixtures more than one STATE-PRODUCING family claimed.
fn contested(
    families: &std::collections::BTreeMap<String, Vec<Fixture>>,
    names: &std::collections::BTreeMap<Fixture, String>,
) -> Vec<StateConflict> {
    let mut conflicts: std::collections::BTreeMap<Fixture, Vec<String>> =
        std::collections::BTreeMap::new();
    for (family, held) in families {
        if !STATE_PRODUCING_FAMILIES.contains(&family.as_str()) {
            continue;
        }
        for fixture in held {
            conflicts
                .entry(fixture.clone())
                .or_default()
                .push(family.clone());
        }
    }
    conflicts
        .into_iter()
        .filter(|(_, claimants)| claimants.len() > 1)
        .map(|(fixture, families)| StateConflict {
            place: names.get(&fixture).cloned().unwrap_or_default(),
            fixture,
            families,
        })
        .collect()
}

/// The two GETs, and the second one only when the claims need it.
pub fn resolve_on_bridge<B: Bridge>(
    bridge: &B,
    families: &std::collections::BTreeMap<String, crate::config::Family>,
) -> Option<Resolution> {
    let rooms = bridge.get("room")?;
    // A LISTING THAT WAS NOT NEEDED AND A LISTING THAT FAILED ARE DIFFERENT
    // ANSWERS, and collapsing them would resolve a config against an empty lamp
    // list: every lamp it names reported as a typo, every room with an
    // exception carved down to nothing, all of it stated confidently about a
    // bridge that said nothing. A GET that was needed and failed is the
    // unreachable bridge, which is a state the doctor already has a line for.
    let lights = if needs_light_listing(families) {
        Some(bridge.get("light")?)
    } else {
        None
    };
    Some(resolve(&rooms, lights.as_deref(), families))
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
    /// Signal every wanted room, and answer with HOW MANY were signalled.
    ///
    /// THE COUNT IS THE ONLY OBSERVABLE FACT ON THIS PATH. `put` is fire and
    /// forget, so a write the bridge refused is invisible; what a caller can
    /// still learn is whether anything was addressed at all, and zero is the
    /// shape both likely misconfigurations take (a bridge that answered no
    /// listing, and a listing in which no configured room name appears).
    pub fn run(&self, exit_code: &str) -> usize {
        let Some(rooms_json) = self.bridge.get("room") else {
            return 0;
        };
        let body = signal_body(crate::pulse::pulse_color(exit_code));
        let grouped = grouped_light_ids_for_rooms(&rooms_json, &self.rooms);
        // INDEPENDENT per group, and every outcome ignored: there is no shared
        // choreography left for a refused write to corrupt, so one room's
        // failure must not cost another its signal, and a failed pulse still
        // never fails the caller.
        for grouped_id in &grouped {
            self.bridge
                .put(&format!("grouped_light/{grouped_id}"), &body);
        }
        grouped.len()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Bridge, DEFAULT_ROOMS, Fixture, HuePulse, Missing, QuietWindow, StateConflict, Unresolved,
        grouped_light_ids_for_rooms, hue_settings, quiet_now, quiet_window, resolve,
        resolve_on_bridge,
    };
    use std::cell::RefCell;
    use std::collections::BTreeMap;

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
        /// The room listing, or None for a bridge that answered nothing: an
        /// unreachable address, a key it refuses, a body that never arrived.
        rooms: Option<&'static str>,
        /// The light listing, answered ONLY to a caller that asked for it, so a
        /// test can see which GETs were really taken.
        lights: Option<&'static str>,
        gets: RefCell<Vec<String>>,
        puts: RefCell<Vec<(String, String)>>,
    }

    impl Bridge for ScriptedBridge {
        fn get(&self, path: &str) -> Option<String> {
            self.gets.borrow_mut().push(path.to_string());
            match path {
                "light" => self.lights.map(String::from),
                _ => self.rooms.map(String::from),
            }
        }
        fn put(&self, path: &str, body: &str) {
            self.puts
                .borrow_mut()
                .push((path.to_string(), body.to_string()));
        }
    }

    fn scripted(rooms: Option<&'static str>) -> ScriptedBridge {
        ScriptedBridge {
            rooms,
            lights: None,
            gets: RefCell::new(Vec::new()),
            puts: RefCell::new(Vec::new()),
        }
    }

    fn bridge() -> ScriptedBridge {
        scripted(Some(ROOMS_JSON))
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
            bridge: scripted(Some(r#"{"data":[]}"#)),
            rooms: wanted(&["3F - Studio"]),
        };
        hue.run("0");
        assert!(hue.bridge.puts.borrow().is_empty());
    }

    #[test]
    fn the_pulse_reports_how_many_rooms_it_signalled() {
        // THE ONLY THING ABOUT THE LIGHTS ANYONE CAN CHECK. The bridge owns
        // the whole effect and acknowledges no write, so the count of rooms
        // that were signalled is the last observable fact on this path; zero
        // is the shape every hue misconfiguration takes.
        assert_eq!(pulse().run("0"), 2, "one signal per matched room");
        assert_eq!(
            HuePulse {
                bridge: scripted(None),
                rooms: wanted(&["3F - Studio"]),
            }
            .run("0"),
            0,
            "a bridge that answered no listing signalled nothing"
        );
        assert_eq!(
            HuePulse {
                bridge: bridge(),
                rooms: wanted(&["1F - Renamed Away"]),
            }
            .run("0"),
            0,
            "and neither did a listing in which no configured name matched"
        );
    }

    // --- the family map ------------------------------------------------------

    // LIFTED FROM THE LIVE CAPTURE of 2026-08-20 (17 lights, 8 rooms), trimmed
    // to the two rooms this repo's own map uses. Every id, name and owner is
    // the bridge's own. NO TEST HERE MAKES A CALL: the listings are literals.
    const CLIP_ROOMS: &str = r#"{"data":[
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
       "services":[{"rid":"b8a1fbd9-829e-42b1-ae3e-c9f24bfcc882","rtype":"grouped_light"}]}
    ]}"#;

    /// The lights in the bridge's own listing order, which is NOT the order the
    /// lamps are named in: HCL3 sits between HCL1 and HCL2.
    ///
    /// The studio's fourth child owns nothing here on purpose: it is a Hue
    /// dimmer SWITCH (`3F - Studio - HDS1`, verified in the same capture), so
    /// the device join has a room member that is not a lamp in it.
    const CLIP_LIGHTS: &str = r#"{"data":[
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

    const STUDIO_GROUP: &str = "14a9dd16-5751-49bc-b33e-701aecf1bce9";
    const HCL1: &str = "17295316-360e-4259-b8fd-928caf1f9c3e";
    const HCL2: &str = "de7b7231-1302-48ed-b0b5-9dd94763d350";
    const HCL3: &str = "9d52d98c-76f0-47d8-a718-4b88cd123665";
    const KITCHEN_HCD3: &str = "0e7a5054-3720-4580-9d8e-8070216e9bfa";

    /// One family's claims, written the way the config's own parser answers
    /// them, so a test states a map rather than a struct literal.
    fn families(written: &str) -> BTreeMap<String, crate::config::Family> {
        crate::config::parse_config(written)
            .expect("the test's own config parses")
            .lights
            .expect("and carries a lights table")
            .families
    }

    fn grouped(id: &str) -> Fixture {
        Fixture::Grouped(id.to_string())
    }

    fn light(id: &str) -> Fixture {
        Fixture::Light(id.to_string())
    }

    #[test]
    fn a_room_claim_resolves_to_its_group_and_a_light_claim_to_that_light() {
        // The two claims are in DIFFERENT rooms on purpose: a light claim
        // INSIDE a claimed room is specific-beats-general, which is its own
        // behaviour two tests down.
        let map = resolve(
            CLIP_ROOMS,
            Some(CLIP_LIGHTS),
            &families(
                "[lights.families.local]\nrooms = [\"3F - Studio\"]\n\
                 [lights.families.github]\nlights = [\"2F - Kitchen - HCD3\"]\n",
            ),
        );
        assert_eq!(
            map.families["local"],
            vec![grouped(STUDIO_GROUP)],
            "a whole room with nothing carved out of it is ONE write to its group"
        );
        assert_eq!(map.families["github"], vec![light(KITCHEN_HCD3)]);
        assert!(map.unresolved.is_empty(), "every name was on the bridge");
    }

    #[test]
    fn a_room_claim_with_an_exception_resolves_to_its_remaining_lights() {
        let map = resolve(
            CLIP_ROOMS,
            Some(CLIP_LIGHTS),
            &families(
                "[lights.families.local]\nrooms = [\"3F - Studio\"]\n\
                 except = [\"3F - Studio - HCL3\"]\n",
            ),
        );
        assert_eq!(
            map.families["local"],
            vec![light(HCL1), light(HCL2)],
            "the join is room children to light owners, the excepted lamp is gone, \
             the group is NOT written (it would light the excepted lamp too), and \
             the room's dimmer switch owns no light so it contributes none"
        );
    }

    #[test]
    fn a_specific_light_claim_beats_the_room_claim_that_contains_it() {
        let map = resolve(
            CLIP_ROOMS,
            Some(CLIP_LIGHTS),
            &families(
                "[lights.families.local]\nrooms = [\"3F - Studio\"]\n\
                 [lights.families.github]\nlights = [\"3F - Studio - HCL3\"]\n\
                 [lights.families.loop]\nlights = [\"3F - Studio - HCL3\"]\n",
            ),
        );
        assert_eq!(
            map.families["local"],
            vec![light(HCL1), light(HCL2)],
            "the room drops to lights because a lamp inside it was claimed by name"
        );
        assert_eq!(map.families["github"], vec![light(HCL3)]);
        assert_eq!(map.families["loop"], vec![light(HCL3)]);
        assert!(
            !map.families["local"].contains(&light(HCL3))
                && !map.families["local"].contains(&grouped(STUDIO_GROUP)),
            "and no fixture of local's can reach the lamp github and loop hold"
        );
    }

    #[test]
    fn a_name_the_bridge_does_not_have_is_reported_with_the_family_that_claimed_it() {
        let map = resolve(
            CLIP_ROOMS,
            Some(CLIP_LIGHTS),
            &families(
                "[lights.families.local]\nlights = [\"3F - Studio - HCL9\"]\n\
                 rooms = [\"3F - Cupboard\"]\n\
                 [lights.families.github]\nlights = [\"3F - Studio - HCL3\"]\n",
            ),
        );
        assert_eq!(
            map.unresolved,
            vec![
                Unresolved {
                    family: "local".to_string(),
                    name: "3F - Cupboard".to_string(),
                    kind: Missing::NotOnBridge,
                },
                Unresolved {
                    family: "local".to_string(),
                    name: "3F - Studio - HCL9".to_string(),
                    kind: Missing::NotOnBridge,
                },
            ],
            "both misses are named, and so is the family that wrote them"
        );
        assert!(
            map.families["local"].is_empty(),
            "a family whose every name missed holds nothing"
        );
        assert_eq!(
            map.families["github"],
            vec![light(HCL3)],
            "and one family's typo costs another family nothing"
        );
    }

    #[test]
    fn a_room_that_addressed_no_lamp_is_reported_per_room_and_in_its_own_words() {
        // PER ROOM, NOT PER FAMILY. The accounting asked whether the FAMILY
        // held anything, so a room that addressed no lamp went unreported the
        // moment any other claim of that family had resolved, and the report it
        // did get called a room the bridge has by name a room the bridge does
        // not have.
        let masked = resolve(
            CLIP_ROOMS,
            Some(CLIP_LIGHTS),
            &families(
                "[lights.families.local]\nlights = [\"2F - Kitchen - HCD3\"]\n\
                 rooms = [\"3F - Studio\"]\n\
                 except = [\"3F - Studio - HCL1\", \"3F - Studio - HCL2\", \
                 \"3F - Studio - HCL3\"]\n",
            ),
        );
        assert_eq!(
            masked.families["local"],
            vec![light(KITCHEN_HCD3)],
            "the kitchen lamp resolved, and a fully excepted room adds nothing to it"
        );
        assert_eq!(
            masked.unresolved,
            vec![Unresolved {
                family: "local".to_string(),
                name: "3F - Studio".to_string(),
                kind: Missing::AddressedNothing,
            }],
            "the room is reported however much else the family holds, and it is \
             reported as what it is: on the bridge, and carved down to nothing"
        );
    }

    #[test]
    fn a_case_folded_claim_is_a_typo_the_doctor_surfaces_rather_than_a_lamp_it_forgives() {
        // NAME MATCHING IS EXACT, and the bridge's spelling is the only
        // spelling. A folding matcher would resolve a wrong-case claim instead
        // of reporting it, and would make two bridge names that differ only in
        // case ambiguous, so the exactness is pinned rather than assumed.
        let map = resolve(
            CLIP_ROOMS,
            Some(CLIP_LIGHTS),
            &families("[lights.families.local]\nlights = [\"3f - studio - hcl3\"]\n"),
        );
        assert_eq!(
            map.unresolved,
            vec![Unresolved {
                family: "local".to_string(),
                name: "3f - studio - hcl3".to_string(),
                kind: Missing::NotOnBridge,
            }],
            "the lamp's own name with the case folded is a name the bridge does not have"
        );
        assert!(
            map.families["local"].is_empty(),
            "and it resolved to nothing: not HCL3, and not anything else"
        );
    }

    #[test]
    fn an_except_the_bridge_does_not_have_costs_the_room_its_group() {
        // FAIL CLOSED. An `except` entry that resolved to no lamp is an
        // operator who asked for a lamp to be left dark and was not told the
        // name missed. Keeping the room's grouped light would write the one
        // lamp the config asked to carve out, which is the exact write the
        // whole-and-untouched rule exists to prevent.
        let missed = resolve(
            CLIP_ROOMS,
            Some(CLIP_LIGHTS),
            &families(
                "[lights.families.local]\nrooms = [\"3F - Studio\"]\n\
                 except = [\"3F - Studio - HCL3 \"]\n",
            ),
        );
        assert_eq!(
            missed.families["local"],
            vec![light(HCL1), light(HCL3), light(HCL2)],
            "a trailing space is a name the bridge does not have, so the room is \
             addressed lamp by lamp and NEVER as its group"
        );
        assert_eq!(
            missed.unresolved,
            vec![Unresolved {
                family: "local".to_string(),
                name: "3F - Studio - HCL3 ".to_string(),
                kind: Missing::NotOnBridge,
            }],
            "and the entry that missed is still named"
        );

        let partly = resolve(
            CLIP_ROOMS,
            Some(CLIP_LIGHTS),
            &families(
                "[lights.families.local]\nrooms = [\"3F - Studio\"]\n\
                 except = [\"3F - Studio - HCL3 \", \"3F - Studio - HCL1\"]\n",
            ),
        );
        assert_eq!(
            partly.families["local"],
            vec![light(HCL3), light(HCL2)],
            "an except that half missed still honours the half that resolved: the \
             room drops to the member lamps, minus the one that was found"
        );
    }

    #[test]
    fn a_light_listing_the_bridge_refused_resolves_nothing_rather_than_everything() {
        // A LISTING THAT WAS NOT NEEDED AND A LISTING THAT FAILED ARE DIFFERENT
        // ANSWERS. Reading a failed GET as an empty lamp list would answer a
        // confident resolution in which every lamp the config named is missing,
        // and the doctor would print a wall of typos for a bridge that simply
        // did not answer.
        let refused = ScriptedBridge {
            rooms: Some(CLIP_ROOMS),
            lights: None,
            gets: RefCell::new(Vec::new()),
            puts: RefCell::new(Vec::new()),
        };
        let map = resolve_on_bridge(
            &refused,
            &families(
                "[lights.families.local]\nrooms = [\"3F - Studio\"]\n\
                 except = [\"3F - Studio - HCL3\"]\n",
            ),
        );
        assert_eq!(
            refused.gets.borrow().as_slice(),
            &["room".to_string(), "light".to_string()],
            "the join was needed, so the second GET was taken"
        );
        assert!(
            map.is_none(),
            "and the GET that answered nothing is the unreachable bridge, not a \
             resolution built on an empty lamp list"
        );
    }

    #[test]
    fn a_family_that_claims_one_lamp_twice_holds_it_once_and_contests_nothing() {
        // A FAMILY CANNOT FIGHT ITSELF. Two spellings of one claim are one
        // claim; counting them twice made the family its own second claimant,
        // which stripped the lamp of the state it was configured to hold.
        let map = resolve(
            CLIP_ROOMS,
            Some(CLIP_LIGHTS),
            &families(
                "[lights.families.local]\n\
                 lights = [\"3F - Studio - HCL3\", \"3F - Studio - HCL3\"]\n",
            ),
        );
        assert_eq!(
            map.families["local"],
            vec![light(HCL3)],
            "one lamp, however many times it was written"
        );
        assert!(
            map.state_conflicts.is_empty(),
            "and no conflict, because there is only one claimant"
        );
        assert_eq!(
            map.state_fixtures("local"),
            vec![light(HCL3)],
            "so the lamp keeps the state the config gave it"
        );
    }

    #[test]
    fn two_state_producing_families_over_one_lamp_are_a_conflict_that_holds_no_state() {
        let map = resolve(
            CLIP_ROOMS,
            Some(CLIP_LIGHTS),
            &families(
                "[lights.families.local]\nlights = [\"3F - Studio - HCL3\"]\n\
                 [lights.families.loop]\nlights = [\"3F - Studio - HCL3\"]\n",
            ),
        );
        assert_eq!(
            map.state_conflicts,
            vec![StateConflict {
                place: "3F - Studio - HCL3".to_string(),
                fixture: light(HCL3),
                families: vec!["local".to_string(), "loop".to_string()],
            }],
            "the lamp is named, and so is every family fighting over it"
        );
        assert!(
            map.state_fixtures("local").is_empty() && map.state_fixtures("loop").is_empty(),
            "and a contested lamp holds no state for either of them"
        );

        let studio_map = resolve(
            CLIP_ROOMS,
            Some(CLIP_LIGHTS),
            &families(
                "[lights.families.github]\nlights = [\"3F - Studio - HCL3\"]\n\
                 [lights.families.loop]\nlights = [\"3F - Studio - HCL3\"]\n",
            ),
        );
        assert!(
            studio_map.state_conflicts.is_empty()
                && studio_map.state_fixtures("loop") == vec![light(HCL3)],
            "the studio's own map is NOT a conflict: github produces no state today, \
             so nothing of its contests the loop lamp"
        );
    }

    #[test]
    fn a_rooms_only_map_costs_one_get_and_a_narrower_one_costs_two() {
        let rooms_only = ScriptedBridge {
            rooms: Some(CLIP_ROOMS),
            lights: Some(CLIP_LIGHTS),
            gets: RefCell::new(Vec::new()),
            puts: RefCell::new(Vec::new()),
        };
        let map = resolve_on_bridge(
            &rooms_only,
            &families("[lights.families.local]\nrooms = [\"3F - Studio\"]\n"),
        )
        .expect("a bridge that listed its rooms resolved");
        assert_eq!(
            rooms_only.gets.borrow().as_slice(),
            &["room".to_string()],
            "THE COST FENCE: a map naming only rooms keeps the pulse path's exact \
             network cost, which is the one GET it has always made"
        );
        assert_eq!(map.families["local"], vec![grouped(STUDIO_GROUP)]);

        let narrower = ScriptedBridge {
            rooms: Some(CLIP_ROOMS),
            lights: Some(CLIP_LIGHTS),
            gets: RefCell::new(Vec::new()),
            puts: RefCell::new(Vec::new()),
        };
        resolve_on_bridge(
            &narrower,
            &families("[lights.families.github]\nlights = [\"3F - Studio - HCL3\"]\n"),
        )
        .expect("and so did this one");
        assert_eq!(
            narrower.gets.borrow().as_slice(),
            &["room".to_string(), "light".to_string()],
            "a map naming a lamp pays for the device join, and only then"
        );
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
