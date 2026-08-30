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
#[derive(Debug, Clone, Copy, PartialEq)]
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

impl Fixture {
    /// The CLIP resource path this fixture is written to.
    ///
    /// WHICH IS THE WHOLE POINT OF THE DISTINCTION. A group write reaches every
    /// lamp in the room, including one a narrower claim carved out; a lamp
    /// write reaches that lamp and nothing else. Addressing either as the other
    /// is a PUT to a resource id of the wrong type, which the bridge answers by
    /// doing nothing and telling no one, because `put` is fire and forget.
    pub fn path(&self) -> String {
        match self {
            Fixture::Grouped(id) => format!("grouped_light/{id}"),
            Fixture::Light(id) => format!("light/{id}"),
        }
    }
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

/// WHERE a fixture is, in the vocabulary the operator writes places in: its
/// own name, and the room holding it when it is a lamp inside one.
///
/// BOTH NAMES, because the settings chain reads them in order. A lamp's own
/// `[lights.places]` entry beats its room's, so knowing only one of the two
/// would make "specific beats general" unstatable for the setting the lamp did
/// not name. A GROUPED fixture IS its room, so it carries no second name: there
/// is nothing more general than the room itself to fall back to.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Placement {
    pub name: String,
    pub room: Option<String>,
}

/// The whole map: what each family holds, where each fixture is, what could not
/// be found, and where two families would fight over one lamp's state.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Resolution {
    pub families: std::collections::BTreeMap<String, Vec<Fixture>>,
    /// Every resolved fixture's place, which is what the settings chain reads.
    /// Resolution is the only thing that knows a lamp's name and its room, and
    /// throwing that away would mean looking both up a second time against a
    /// listing that may have changed underneath.
    pub places: std::collections::BTreeMap<Fixture, Placement>,
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

/// The family every event on the notification path routes to.
///
/// IT LIVES BESIDE THE OTHER TWO FAMILY LISTS because it is the same fact:
/// which names this crate speaks. Its failure mode is the quietest one in the
/// slice, a family holding no fixtures, `signal_family` returning at once, and
/// every lamp dark for good with nothing said on stdout, in the doctor or in a
/// log, so the composition root reads it from here rather than keeping a
/// literal of its own.
pub const LOCAL_FAMILY: &str = "local";

/// Every family name pns speaks: the two `STATE_PRODUCING_FAMILIES`, plus
/// `github`, which resolves lamps that nothing routes to yet (operator ruling:
/// PLUMBING ONLY).
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
    // THE SAME DEVICE JOIN THE ROOM CLAIMS RUN, taken once for every lamp, so a
    // lamp claimed BY NAME still knows which room it is in. Without it the
    // settings chain would have a room to fall back to only for lamps that
    // happened to be reached through a room claim, which is the same setting
    // resolving two different ways depending on how the operator spelled the
    // map.
    let room_of = room_membership(&rooms, &lamps);

    let mut resolution = Resolution::default();
    let mut spoken_for: Vec<String> = Vec::new();

    // PASS ONE, every light claim, because pass two subtracts them.
    for (family, claims) in families {
        let mut held = Vec::new();
        for name in &claims.lights {
            match named(name) {
                Some(lamp) => {
                    let fixture = Fixture::Light(lamp.id.clone());
                    resolution
                        .places
                        .insert(fixture.clone(), placement(lamp, &room_of));
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
            let places = &mut resolution.places;
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
                    places.insert(
                        fixture.clone(),
                        Placement {
                            name: room_name.clone(),
                            room: None,
                        },
                    );
                    held.push(fixture);
                }
                _ => {
                    for lamp in kept {
                        let fixture = Fixture::Light(lamp.id.clone());
                        places.insert(fixture.clone(), placement(lamp, &room_of));
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
    resolution.state_conflicts = contested(&resolution.families, &resolution.places);
    resolution
}

/// Which room each lamp is in, by the device join: a room lists DEVICE rids as
/// its children, and a lamp names its device in `owner.rid`.
fn room_membership(
    rooms: &[serde_json::Value],
    lamps: &[Lamp],
) -> std::collections::BTreeMap<String, String> {
    let mut membership = std::collections::BTreeMap::new();
    for room in rooms {
        let Some(name) = room
            .pointer("/metadata/name")
            .and_then(|name| name.as_str())
        else {
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
        for lamp in lamps
            .iter()
            .filter(|lamp| children.contains(&lamp.owner.as_str()))
        {
            // FIRST ROOM WINS, and a lamp in two rooms is not a shape the
            // bridge produces: a light belongs to one room. Overwriting would
            // make the answer depend on listing order.
            membership
                .entry(lamp.id.clone())
                .or_insert_with(|| name.to_string());
        }
    }
    membership
}

/// One lamp's place: its own name, and the room the join put it in.
fn placement(lamp: &Lamp, room_of: &std::collections::BTreeMap<String, String>) -> Placement {
    Placement {
        name: lamp.name.clone(),
        room: room_of.get(&lamp.id).cloned(),
    }
}

/// The fixtures more than one STATE-PRODUCING family claimed.
fn contested(
    families: &std::collections::BTreeMap<String, Vec<Fixture>>,
    places: &std::collections::BTreeMap<Fixture, Placement>,
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
            place: places
                .get(&fixture)
                .map(|placement| placement.name.clone())
                .unwrap_or_default(),
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

/// The PUT body one behaviour signals with, or None for a behaviour that has
/// no pulse shape yet.
///
/// A BEHAVIOUR IS (SIGNAL TYPE, COLOURS, DURATION), and that triple is the
/// whole vocabulary. `on_off_color` toggles between off and one colour;
/// `alternating` runs between two. Both expire on their own, which is what
/// makes every failure on this path resolve to a dark lamp.
///
/// `Breathing` and `Glow` answer NONE, and it is a fact rather than a gap.
/// They are the daemon's states, their durations are a function of the
/// `refresh_secs` this one-shot path never reads, and what either looks like at
/// twenty to sixty seconds has not been observed on a lamp (drill D3). A shape
/// invented here would be one nobody measured, and answering None leaves the
/// lamp dark rather than arming it wrongly.
///
/// The bridge OWNS THE WHOLE EFFECT: it flashes the colour for the duration and
/// then puts the lamp back exactly as it was, with no snapshot, no restore
/// writes and no choreography from us. That is why this channel is one PUT.
///
/// MEASURED ON 2026-09-01, on a real lamp, in both directions: a full state
/// read before and after a signal came back byte-identical with the lamp on
/// and with it off. Before that drill this comment asserted the restore with
/// nothing behind it, and the specification is silent on the question.
///
/// IT IS TRUE OF A SIGNAL AND OF NOTHING ELSE. `state_body`'s glow is a plain
/// state write rather than a signal, so no restore is coming for it and it
/// carries two explicit clears instead.
pub fn signal_body(behaviour: crate::config::Behaviour) -> Option<String> {
    let (signal, colors) = match behaviour {
        crate::config::Behaviour::Done => ("on_off_color", vec![crate::pulse::SUCCESS_COLOR]),
        crate::config::Behaviour::Failed => ("on_off_color", vec![crate::pulse::FAILURE_COLOR]),
        crate::config::Behaviour::NeedsYou => (
            "alternating",
            vec![
                crate::pulse::NEEDS_YOU_COLOR,
                crate::pulse::NEEDS_YOU_ALT_COLOR,
            ],
        ),
        crate::config::Behaviour::Breathing | crate::config::Behaviour::Glow => return None,
    };
    Some(
        serde_json::json!({
            "signaling": {
                "signal": signal,
                "duration": SIGNAL_DURATION_MS,
                "colors": colors
                    .iter()
                    .map(|color| serde_json::json!({"xy": {"x": color.x, "y": color.y}}))
                    .collect::<Vec<_>>(),
            },
        })
        .to_string(),
    )
}

/// Whether one fixture signals this behaviour right now.
///
/// THE SETTINGS CHAIN IS RESOLVED PER SETTING, not wholesale: the lamp's own
/// `[lights.places]` entry, then its room's entry, then what is left. A lamp
/// that named only a `skip` list still inherits its room's quiet hours, which
/// an entry-shaped chain would have taken away the moment the lamp wrote one
/// key.
///
/// AN EMPTY `skip` LIST IS AN ABSENT ONE, and that is a stated limit rather
/// than a rule. A `Vec` cannot tell "the operator wrote no skip list" from "the
/// operator wrote an empty one", so a lamp has no spelling for overriding its
/// room's skip list back to nothing. Naming it here because a reader will
/// otherwise find it by surprise.
pub fn place_signals(
    lights: &crate::config::Lights,
    placement: &Placement,
    behaviour: crate::config::Behaviour,
    fallback: Result<Option<&QuietWindow>, &str>,
    minutes_now: Option<u16>,
) -> Result<bool, String> {
    let chain = place_chain(lights, placement);
    if skipped(&chain, behaviour) {
        return Ok(false);
    }
    Ok(!quiet_now(
        place_window(&chain, fallback)?.as_ref(),
        minutes_now,
    ))
}

/// Whether one fixture shows a STATE right now, which is `place_signals`'s
/// question plus the catch-up one.
///
/// THE EXTRA QUESTION ONLY A STATE CAN BE ASKED. A pulse describes a moment
/// that is already gone, so there is nothing to catch up on; a state persists,
/// and one that began inside a place's quiet window would otherwise appear the
/// instant the window ended. The operator's ruling is that it does NOT, unless
/// that place opted in.
///
/// ONE CHAIN WALK FOR BOTH, which is why this is a sibling of `place_signals`
/// rather than a second call after it: two walks would parse the same window
/// twice and refuse it twice.
///
/// `catch_up` IS THE FIRST RUNG THAT STATED IT, which is the same specific-first
/// rule the window walk below follows and for the same reason: a lamp that wrote
/// `false` is turning its room's catch-up back off, and nothing else it could
/// have meant. Silence at a rung is not a `false` there, so writing one key at a
/// lamp never takes the room's other settings away.
///
/// A START THIS MACHINE CANNOT PLACE IN THE DAY FAILS TOWARD DARK, through
/// `quiet_now`'s own unreadable-clock rule: an unplaceable start inside a
/// configured window is treated as having been inside it.
pub fn place_shows_state(
    lights: &crate::config::Lights,
    placement: &Placement,
    behaviour: crate::config::Behaviour,
    fallback: Result<Option<&QuietWindow>, &str>,
    minutes_now: Option<u16>,
    started_minutes: Option<u16>,
) -> Result<bool, String> {
    let chain = place_chain(lights, placement);
    if skipped(&chain, behaviour) {
        return Ok(false);
    }
    let window = place_window(&chain, fallback)?;
    if quiet_now(window.as_ref(), minutes_now) {
        return Ok(false);
    }
    if chain
        .iter()
        .find_map(|(_, place)| place.catch_up)
        .unwrap_or(false)
    {
        return Ok(true);
    }
    Ok(!quiet_now(window.as_ref(), started_minutes))
}

/// The place chain, SPECIFIC FIRST: the lamp's own entry, then its room's.
///
/// EACH RUNG CARRIES THE NAME IT WAS WRITTEN UNDER, because a refusal has to
/// name the ENTRY that is wrong rather than the lamp that read it: a room's
/// unreadable window darkens every lamp in the room, and sending the operator
/// to a lamp's own entry to fix a typo in the room's is a message they act on
/// and get nowhere with.
fn place_chain<'settings>(
    lights: &'settings crate::config::Lights,
    placement: &'settings Placement,
) -> Vec<(&'settings str, &'settings crate::config::Place)> {
    [Some(placement.name.as_str()), placement.room.as_deref()]
        .iter()
        .flatten()
        .filter_map(|name| Some((*name, lights.places.get(*name)?)))
        .collect()
}

/// Whether the chain refuses this behaviour: the first rung that stated a skip
/// list at all is the one that decides.
fn skipped(chain: &[(&str, &crate::config::Place)], behaviour: crate::config::Behaviour) -> bool {
    chain
        .iter()
        .map(|(_, place)| &place.skip)
        .find(|skip| !skip.is_empty())
        .is_some_and(|skip| skip.contains(&behaviour))
}

/// The quiet window that applies, walking the SAME chain from the start.
///
/// A SECOND WALK IS WHAT "PER SETTING" MEANS: a lamp that named only a skip
/// list has said nothing about hours, so its room's window is still the one
/// that applies, which an entry-shaped chain would have taken away the moment
/// the lamp wrote one key.
///
/// AND `[plugins.hue] quiet_hours` IS THE LAST RUNG rather than a gate in
/// front of the whole pulse, which is where it used to fail closed. A place
/// that stated hours of its own never reads the house key at all, so a typo
/// there costs exactly the places that reached this rung, and the refusal
/// names the house key because that is the entry the operator has to fix.
///
/// FAIL CLOSED, FOR THIS PLACE ALONE, on an unreadable one. An operator who
/// asked for quiet hours and mistyped them would otherwise be flashed at 3am
/// and told nothing, which is `quiet_window`'s own argument one level down;
/// what is new is that the cost is one lamp rather than the whole house.
fn place_window(
    chain: &[(&str, &crate::config::Place)],
    fallback: Result<Option<&QuietWindow>, &str>,
) -> Result<Option<QuietWindow>, String> {
    let Some((wrote_it, stated)) = chain
        .iter()
        .find_map(|(name, place)| Some((*name, place.quiet_hours.as_deref()?)))
    else {
        return fallback
            .map(|window| window.copied())
            .map_err(str::to_string);
    };
    parse_window(stated)
        .map(Some)
        .ok_or_else(|| place_hours_refusal(wrote_it, stated))
}

/// The refusal, in the shape `quiet_hours_refusal` already uses one level up,
/// with the PLACE in it: an operator standing in a dark room cannot tell a lamp
/// that refused from a lamp that is asleep, and a message that names neither
/// the lamp nor what was written is not one they can act on.
fn place_hours_refusal(place: &str, stated: &str) -> String {
    format!(
        "pns: config error (lights.places.{place:?} quiet_hours is {stated:?}, \
         not a HH:MM-HH:MM window); that place stays dark"
    )
}

/// The bridge seam: authenticated GETs and PUTs against the CLIP paths.
pub trait Bridge {
    fn get(&self, path: &str) -> Option<String>;
    /// Fire and forget: `run` discards every outcome, so a bridge that
    /// refuses tells no one. Returning a result would be a seam with no
    /// consumer.
    fn put(&self, path: &str, body: &str);
}

/// Whether any fixture the ROUTED family could hold might signal now,
/// answered from the CONFIG ALONE.
///
/// THE INVARIANT: false only where the config says that every fixture this
/// family could hold is quiet at this minute. Anything the config cannot judge
/// answers true, because a wrong true costs one bridge round trip that writes
/// nothing and a wrong false is a lamp that never lights with no message
/// anywhere.
///
/// THE POINT IS THE CALL THAT IS NOT MADE. Resolving fixtures costs a bridge
/// GET, and a house that is entirely asleep has nothing to resolve them for: an
/// event at 3am with every window covering it should reach no network at all,
/// which is what the shipped whole-pulse gate gave and what a per-fixture gate
/// would otherwise have taken away.
///
/// ONE FAMILY, because the gate stands in front of ONE family's signal. A
/// `github` or `loop` place awake at 3am says nothing about whether the event
/// being routed to `local` can light anything, and an entry no claim names at
/// all bought a round trip for a room this pulse was never going to address.
///
/// AN UNREADABLE WINDOW COUNTS AS AWAKE, the house key's included. A config
/// this cannot judge is not a config proven dark, and the refusal that names
/// the entry is printed only by the walk that visits the fixtures, so this gate
/// has to err loud for that walk to be reached at all.
///
/// TWO LIMITS, both of them the price of answering with no GET:
///
/// - A CLAIMED ROOM IS JUDGED BY ITS OWN ENTRY, and the config alone cannot say
///   which lamps that room holds; the device join that could is the GET this
///   exists to avoid. So a lamp that carved an AWAKE window out of a sleeping
///   room is not seen here and loses its signal to this gate. The alternative
///   is to treat every entry nobody claimed as a possible member of every
///   claimed room, which is the scan this replaced: it bought a round trip for
///   any stranger entry in the table, including one whose own typo the walk
///   then never reached to report.
/// - A CLAIMED LIGHT WITH NO HOURS OF ITS OWN takes the house window here,
///   where the walk would try its ROOM's entry first. A lamp in a sleeping room
///   under a loud house therefore reads awake to this gate and dark to the
///   walk, which costs one round trip and no wrong light.
pub fn any_place_loud(
    lights: &crate::config::Lights,
    family: &str,
    fallback: Result<Option<&QuietWindow>, &str>,
    minutes_now: Option<u16>,
) -> bool {
    // A FAMILY THAT CLAIMED NOTHING HOLDS NOTHING, and `signal_family` would
    // walk an empty list however awake the house is.
    let Some(claims) = lights.families.get(family) else {
        return false;
    };
    claims.rooms.iter().chain(&claims.lights).any(|name| {
        let Some(stated) = lights
            .places
            .get(name)
            .and_then(|place| place.quiet_hours.as_deref())
        else {
            // THE SAME LAST RUNG `place_signals` walks to, and the same
            // fail-loud reading of a house key nobody can parse.
            return match fallback {
                Ok(window) => !quiet_now(window, minutes_now),
                Err(_) => true,
            };
        };
        parse_window(stated).is_none_or(|window| !quiet_now(Some(&window), minutes_now))
    })
}

/// The event path's signal: one PUT per fixture the family holds, each one
/// filtered by its own place's tables, and the refusals nobody could act on
/// silently.
///
/// PER FIXTURE, WHICH IS THE WHOLE POINT. One lamp refusing a behaviour, or
/// asleep, or carrying an unreadable window, must cost that lamp its signal and
/// no other. The gate used to answer one question for the whole pulse, which
/// meant one typo took the house dark.
///
/// IT PRINTS NOTHING. The refusals come back as text and the composition root
/// decides where they go, which is the rule every module in this crate follows.
pub fn signal_family<B: Bridge>(
    bridge: &B,
    resolution: &Resolution,
    family: &str,
    lights: &crate::config::Lights,
    behaviour: crate::config::Behaviour,
    fallback: Result<Option<&QuietWindow>, &str>,
    minutes_now: Option<u16>,
) -> Vec<String> {
    let Some(body) = signal_body(behaviour) else {
        return Vec::new();
    };
    let mut refusals = Vec::new();
    let Some(fixtures) = resolution.families.get(family) else {
        return refusals;
    };
    for fixture in fixtures {
        // A FIXTURE WITH NO PLACEMENT IS A LAMP RESOLUTION DID NOT NAME, which
        // cannot happen through `resolve` and would mean an empty settings
        // chain if it did. The default carries no name and no room, so it
        // matches no `[lights.places]` entry and takes the fallback window,
        // which is the same treatment a lamp the operator never mentioned gets.
        let placement = resolution.places.get(fixture).cloned().unwrap_or_default();
        match place_signals(lights, &placement, behaviour, fallback, minutes_now) {
            Ok(true) => bridge.put(&fixture.path(), &body),
            Ok(false) => {}
            // ONE REFUSAL PER PLACE, not per lamp that reached it: two lamps
            // inheriting one room's unreadable window is one typo, and saying
            // it twice trains an operator to skim the line.
            Err(refusal) => {
                if !refusals.contains(&refusal) {
                    refusals.push(refusal);
                }
            }
        }
    }
    refusals
}

/// The family that holds the loop lamp's two states.
///
/// BESIDE `LOCAL_FAMILY` AND `KNOWN_FAMILIES`, because it is the same fact:
/// which names this crate speaks. `STATE_PRODUCING_FAMILIES` is the pair of
/// them and is what a conflict is judged against.
pub const LOOP_FAMILY: &str = "loop";

/// Whether a family produces this behaviour as a STATE.
///
/// THE FAMILY IS WHERE A BEHAVIOUR LIVES, and the map is the operator's
/// vocabulary: `local` is the agents at this desk, so a wait is theirs;
/// `loop` is the work itself, so breathing and glowing are its. A lamp
/// claimed by neither producer of the current state is simply dark, which is
/// how one house state reaches three lamps saying different things.
///
/// A PULSE IS NOT A STATE, so `done` and `failed` answer false for every
/// family. They fire once from the event path and nothing re-arms them; a
/// family "producing" one would be a lamp the tick kept flashing green.
pub fn family_produces(family: &str, behaviour: crate::config::Behaviour) -> bool {
    producing_family(behaviour) == Some(family)
}

/// Which family produces this behaviour as a state, or None for a pulse.
///
/// THE ONE MAPPING, and `family_produces` is written in terms of it so the
/// walk that arms a lamp and the gate that decides whether any lamp could be
/// awake cannot come out disagreeing about one behaviour.
pub fn producing_family(behaviour: crate::config::Behaviour) -> Option<&'static str> {
    match behaviour {
        crate::config::Behaviour::NeedsYou => Some(LOCAL_FAMILY),
        crate::config::Behaviour::Breathing | crate::config::Behaviour::Glow => Some(LOOP_FAMILY),
        crate::config::Behaviour::Done | crate::config::Behaviour::Failed => None,
    }
}

/// What is wrong with one claim, in one sentence and with no prefix on it.
///
/// ONE WORDING, TWO READERS. The doctor prefixes it with its own name and the
/// tick with its own, and an operator who reads the same lamp reported two
/// different ways has to work out whether they are the same problem.
pub fn missing_sentence(missing: &Unresolved) -> String {
    match missing.kind {
        Missing::NotOnBridge => format!(
            "lights: `{}` ({}) is not on the bridge",
            missing.name, missing.family
        ),
        // A DIFFERENT JOB, SO A DIFFERENT SENTENCE: this name IS on the
        // bridge, and telling the operator to go find it would send them
        // looking for something already in front of them.
        Missing::AddressedNothing => format!(
            "lights: `{}` ({}) is on the bridge, but that claim addressed no lamp",
            missing.name, missing.family
        ),
    }
}

/// How much longer than one refresh interval a state's signal runs.
///
/// THE OVERLAP IS THE POINT. A signal that expired exactly at the refresh
/// would leave the lamp dark for however long the next tick took to arrive,
/// which is a flicker on every interval; a few seconds of overlap means the
/// re-arm lands while the old signal is still running. D1 measured on
/// 2026-09-01 that a second signalling PUT cleanly REPLACES a running one and
/// restarts its duration, so the overlap costs nothing.
const STATE_SLACK_SECS: u64 = 5;

/// The longest duration the bridge accepts, in milliseconds
/// (`Signaling.yaml`, verified 2026-09-01). A refresh interval near its own
/// ceiling would otherwise compute a duration the bridge refuses, and a
/// refused PUT is a dark lamp with nothing said anywhere.
const MAX_SIGNAL_DURATION_MS: u64 = 65_534_000;

/// The bridge's own breathe: a smooth swell it renders and ends itself.
///
/// THE V2 `alert` ACTION, which every light and grouped_light in the live
/// capture exposes. It is deliberately the one thing on this path with no
/// duration of ours on it.
const ALERT_BREATHE: &str = "breathe";

/// How bright the glow runs, in percent. Low enough to read as a glow rather
/// than as a lamp somebody left on, and not yet approved as seen: it waits on
/// the operator's eye exactly as the colours do.
const GLOW_BRIGHTNESS: f64 = 25.0;

/// The PUT body one STATE holds a lamp with, or None for a behaviour that is a
/// pulse rather than a state.
///
/// THREE SHAPES, AND THE DRILL OF 2026-09-01 CHOSE TWO OF THEM.
///
/// `needs-you` alternates its two deep blues for one refresh interval plus the
/// slack, so the daemon's next re-arm lands while it is still running.
///
/// `breathing` IS THE BRIDGE'S OWN BREATHE (operator decision, 2026-08-30).
/// Every shape this crate could build out of `signalling` failed on a real
/// lamp: a long `on_off_color` is a strobe, and a near-steady alternating pair
/// is a turn signal. The bridge already renders what was wanted, so this asks
/// for it and adds nothing: a smooth swell around whatever colour the lamp is
/// currently showing, which is why no colour of ours appears in this body.
///
/// IT ENDS ITSELF after about fifteen seconds, so breathing keeps the
/// fail-to-dark property the rest of this path has: a dead daemon, a dead
/// network and a dead pns all leave the lamp where it started. The tick
/// re-sends it every `refresh_secs` while the condition holds, which is why a
/// refresh above that fifteen seconds leaves visible gaps between swells; the
/// config template says so where an operator sets the number.
///
/// NO DURATION IS COMPUTED HERE. How long a swell runs is the bridge's
/// business, and a duration field on this action is one the bridge ignores.
///
/// `glow` IS THE ONE BODY HERE THAT IS NOT A SIGNAL AT ALL. The drill found
/// the near-steady alternating pair read as a TURN SIGNAL, so glow takes the
/// design's own plan B: a plain state write of `on` plus `color` plus a low
/// `dimming`, which is genuinely steady. THE PRICE, STATED WHERE IT IS PAID:
/// this write does NOT expire, so glow alone loses the fail-to-dark property
/// every other body on this path has. A daemon that dies holding it leaves the
/// lamp lit. It is paid for with two explicit clears, one from the return
/// moment on the event path (which needs no daemon) and one from any tick that
/// sees the condition gone, and `signal_state` reports which fixtures are held
/// so both clears have names to write to.
pub fn state_body(behaviour: crate::config::Behaviour, refresh_secs: u64) -> Option<String> {
    let (signal, colors, duration_ms) = match behaviour {
        crate::config::Behaviour::NeedsYou => (
            "alternating",
            vec![
                crate::pulse::NEEDS_YOU_COLOR,
                crate::pulse::NEEDS_YOU_ALT_COLOR,
            ],
            refresh_secs
                .saturating_add(STATE_SLACK_SECS)
                .saturating_mul(1000)
                .min(MAX_SIGNAL_DURATION_MS),
        ),
        crate::config::Behaviour::Breathing => {
            return Some(serde_json::json!({"alert": {"action": ALERT_BREATHE}}).to_string());
        }
        crate::config::Behaviour::Glow => {
            return Some(
                serde_json::json!({
                    "on": {"on": true},
                    "color": {"xy": {"x": crate::pulse::LOOP_COLOR.x, "y": crate::pulse::LOOP_COLOR.y}},
                    "dimming": {"brightness": GLOW_BRIGHTNESS},
                })
                .to_string(),
            );
        }
        crate::config::Behaviour::Done | crate::config::Behaviour::Failed => return None,
    };
    Some(
        serde_json::json!({
            "signaling": {
                "signal": signal,
                "duration": duration_ms,
                "colors": colors
                    .iter()
                    .map(|color| serde_json::json!({"xy": {"x": color.x, "y": color.y}}))
                    .collect::<Vec<_>>(),
            },
        })
        .to_string(),
    )
}

/// What puts a steadily-held lamp out.
///
/// OFF, AND NOT A RESTORE. Nothing snapshotted what the lamp was doing before
/// the glow took it, and a grouped_light GET carries no colour at all, so
/// there is nothing honest to put back. Dark is what "the state is over" means
/// everywhere else on this path, and the operator's own ruling is that pns
/// animates in-use lamps.
pub fn clear_body() -> String {
    serde_json::json!({"on": {"on": false}}).to_string()
}

/// The state this tick is arming, and the clock readings it is judged against.
///
/// ONE NAMED VALUE rather than five loose arguments, three of which are
/// clock-shaped: a transposition between two of those would be a lamp judged
/// against the wrong minute and nothing would catch it.
pub struct Arming<'reading> {
    pub behaviour: crate::config::Behaviour,
    /// How often the daemon comes back, which is what a state's own duration
    /// is sized against.
    pub refresh_secs: u64,
    /// The house window, read but not judged: it is the last rung of the
    /// per-place chain, exactly as it is on the pulse path.
    pub fallback: Result<Option<&'reading QuietWindow>, &'reading str>,
    pub minutes_now: Option<u16>,
    /// The minute of the day the state BEGAN, which is the only thing the
    /// catch-up rule reads.
    pub started_minutes: Option<u16>,
}

/// What one tick's arming left behind.
pub struct StateWrite {
    /// The place refusals, deduplicated, for the caller to decide what to do
    /// with. This module prints nothing, like every other one in this crate.
    pub refusals: Vec<String>,
    /// EVERY fixture path this arming wrote to, whatever the body was.
    ///
    /// IT IS WHAT A CLEAR HAS TO SUBTRACT. A caller putting out the paths an
    /// earlier steady write is holding must not put out one this arming has
    /// just written to: glow and breathing are produced by the same family, so
    /// the breathe and the off would reach the same lamp in that order and
    /// leave it dark. `held` below is a SUBSET of this.
    pub signalled: Vec<String>,
    /// The fixture paths written with a body that does NOT expire, which is
    /// glow and only glow. The caller records these so a later clear has names
    /// to write to; an empty list is a tick that left nothing behind.
    pub held: Vec<String>,
}

/// The tick's writes: one state body per fixture whose family produces the
/// state and whose own place lets it through.
///
/// EVERY STATE-PRODUCING FAMILY IS WALKED, and `state_fixtures` is what drops
/// a lamp two of them are fighting over. A contested lamp holds NO state
/// rather than going to whichever family the walk reached first.
///
/// PER FIXTURE, exactly as `signal_family` is and for the same reason: one
/// lamp asleep, refusing the behaviour, or carrying an unreadable window must
/// cost that lamp its state and no other.
///
/// IT PRINTS NOTHING and it reads no clock: the composition root hands in the
/// minute and decides where the refusals go.
pub fn signal_state<B: Bridge>(
    bridge: &B,
    resolution: &Resolution,
    lights: &crate::config::Lights,
    arming: &Arming<'_>,
) -> StateWrite {
    let mut written = StateWrite {
        refusals: Vec::new(),
        signalled: Vec::new(),
        held: Vec::new(),
    };
    let Some(body) = state_body(arming.behaviour, arming.refresh_secs) else {
        return written;
    };
    let holds = arming.behaviour == crate::config::Behaviour::Glow;
    for family in STATE_PRODUCING_FAMILIES {
        if !family_produces(family, arming.behaviour) {
            continue;
        }
        for fixture in resolution.state_fixtures(family) {
            let placement = resolution.places.get(&fixture).cloned().unwrap_or_default();
            match place_shows_state(
                lights,
                &placement,
                arming.behaviour,
                arming.fallback,
                arming.minutes_now,
                arming.started_minutes,
            ) {
                Ok(true) => {
                    bridge.put(&fixture.path(), &body);
                    written.signalled.push(fixture.path());
                    if holds {
                        written.held.push(fixture.path());
                    }
                }
                Ok(false) => {}
                // ONE REFUSAL PER PLACE, not per lamp that reached it, which is
                // `signal_family`'s own rule: two lamps inheriting one room's
                // unreadable window is one typo.
                Err(refusal) => {
                    if !written.refusals.contains(&refusal) {
                        written.refusals.push(refusal);
                    }
                }
            }
        }
    }
    written
}

/// Put out every lamp a steady write is still holding.
///
/// OFF THE HELD PATHS ALONE, with no listing resolved: the paths were recorded
/// when they were written, so a clear costs no GET and cannot be defeated by a
/// bridge that has stopped answering its `room` listing. That is what lets the
/// EVENT path make this call with no daemon involved at all.
pub fn clear_held<B: Bridge>(bridge: &B, held: &[String]) {
    let body = clear_body();
    for path in held {
        bridge.put(path, &body);
    }
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
    pub fn run(&self, behaviour: crate::config::Behaviour) -> usize {
        let Some(rooms_json) = self.bridge.get("room") else {
            return 0;
        };
        let fixtures: Vec<Fixture> = grouped_light_ids_for_rooms(&rooms_json, &self.rooms)
            .into_iter()
            .map(Fixture::Grouped)
            .collect();
        signal_fixtures(&self.bridge, &fixtures, behaviour)
    }
}

/// One PUT per fixture, addressed by WHAT EACH ONE IS, and how many were
/// written.
///
/// INDEPENDENT per fixture, and every outcome ignored: there is no shared
/// choreography left for a refused write to corrupt, so one lamp's failure must
/// not cost another its signal, and a failed pulse still never fails the caller.
///
/// A BEHAVIOUR WITH NO BODY WRITES NOTHING AT ALL rather than falling back to
/// one that has one. A lamp asked to breathe would otherwise flash whatever
/// shape was nearest, which is the lying lamp this whole design exists to
/// prevent.
pub fn signal_fixtures<B: Bridge>(
    bridge: &B,
    fixtures: &[Fixture],
    behaviour: crate::config::Behaviour,
) -> usize {
    let Some(body) = signal_body(behaviour) else {
        return 0;
    };
    for fixture in fixtures {
        bridge.put(&fixture.path(), &body);
    }
    fixtures.len()
}

#[cfg(test)]
mod tests {
    use super::{
        Arming, Bridge, DEFAULT_ROOMS, Fixture, HuePulse, LOCAL_FAMILY, Missing, Placement,
        QuietWindow, Resolution, StateConflict, Unresolved, any_place_loud, clear_held,
        grouped_light_ids_for_rooms, hue_settings, place_shows_state, quiet_now, quiet_window,
        resolve, resolve_on_bridge, signal_body, signal_family, signal_state, state_body,
    };
    use crate::config::Behaviour;
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
    /// The needs-you body, which has never shipped: two colours and the
    /// `alternating` signal, so it cannot be mistaken for either of the two
    /// above at a glance.
    const BLUE_SIGNAL: &str = r#"{"signaling":{"colors":[{"xy":{"x":0.1532,"y":0.0475}},{"xy":{"x":0.15,"y":0.06}}],"duration":3000,"signal":"alternating"}}"#;

    #[test]
    fn a_failure_signals_every_wanted_room_red_and_writes_nothing_else() {
        let hue = pulse();
        hue.run(Behaviour::Failed);
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
        hue.run(Behaviour::Done);
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
        hue.run(Behaviour::Failed);
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
        hue.run(Behaviour::Done);
        assert!(hue.bridge.puts.borrow().is_empty());
    }

    #[test]
    fn the_pulse_reports_how_many_rooms_it_signalled() {
        // THE ONLY THING ABOUT THE LIGHTS ANYONE CAN CHECK. The bridge owns
        // the whole effect and acknowledges no write, so the count of rooms
        // that were signalled is the last observable fact on this path; zero
        // is the shape every hue misconfiguration takes.
        assert_eq!(
            pulse().run(Behaviour::Done),
            2,
            "one signal per matched room"
        );
        assert_eq!(
            HuePulse {
                bridge: scripted(None),
                rooms: wanted(&["3F - Studio"]),
            }
            .run(Behaviour::Done),
            0,
            "a bridge that answered no listing signalled nothing"
        );
        assert_eq!(
            HuePulse {
                bridge: bridge(),
                rooms: wanted(&["1F - Renamed Away"]),
            }
            .run(Behaviour::Done),
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

    // --- the vocabulary ------------------------------------------------------

    #[test]
    fn the_green_and_red_bodies_are_byte_for_byte_what_shipped() {
        // A GUARD, not a red-first test, and it says so here rather than in a
        // report nobody reads beside the code. The behaviour vocabulary
        // replaces an exit code and a colour with a five-word enum, and the two
        // bodies an operator has been looking at since 2026-08-12 must not move
        // one byte in the process. Its job is to keep passing.
        assert_eq!(
            signal_body(Behaviour::Done).as_deref(),
            Some(GREEN_SIGNAL),
            "done is the shipped green body"
        );
        assert_eq!(
            signal_body(Behaviour::Failed).as_deref(),
            Some(RED_SIGNAL),
            "failed is the shipped red body"
        );
    }

    #[test]
    fn needs_you_alternates_between_two_colours_rather_than_flashing_one() {
        // THE PATTERN IS PART OF THE BEHAVIOUR, not decoration on top of a
        // colour. Blue on `on_off_color` would read as a green pulse from
        // across the room at a glance, which is the one confusion this
        // behaviour exists to prevent: green says it finished, blue says it is
        // waiting on you.
        assert_eq!(
            signal_body(Behaviour::NeedsYou).as_deref(),
            Some(BLUE_SIGNAL)
        );
    }

    #[test]
    fn the_two_state_behaviours_carry_no_body_until_a_lamp_has_been_watched() {
        // FAIL TO DARK, and it is the honest answer rather than a placeholder.
        // Breathing and glow are the daemon's, their durations are a function
        // of `refresh_secs` this pulse never reads, and what they LOOK like at
        // twenty to sixty seconds has never been observed on a bulb (drill D3).
        // A body invented here would be a shape nobody measured, arming a lamp
        // on a schedule that does not exist yet.
        assert_eq!(signal_body(Behaviour::Breathing), None);
        assert_eq!(signal_body(Behaviour::Glow), None);
    }

    #[test]
    fn a_grouped_fixture_addresses_a_group_and_a_light_addresses_a_light() {
        // The CLIP path is what makes a fixture a fixture: a room's write goes
        // to its grouped light and reaches every lamp in it, and a lamp's write
        // goes to that lamp and reaches nothing else. Addressing one as the
        // other is a write to a resource id of the wrong type.
        assert_eq!(
            grouped(STUDIO_GROUP).path(),
            format!("grouped_light/{STUDIO_GROUP}")
        );
        assert_eq!(light(HCL1).path(), format!("light/{HCL1}"));
    }

    // --- per place: what a lamp refuses, and when it is asleep ---------------

    /// The whole `[lights]` table a test states, parsed by its own parser.
    fn lights(written: &str) -> crate::config::Lights {
        *crate::config::parse_config(written)
            .expect("the test's own config parses")
            .lights
            .expect("and carries a lights table")
    }

    /// The studio carved down to HCL1 and HCL2, which is the repo's own map:
    /// the two lamps `local` holds once HCL3 is handed to the other families.
    const STUDIO_LOCAL: &str = "[lights.families.local]\nrooms = [\"3F - Studio\"]\n\
         except = [\"3F - Studio - HCL3\"]\n";

    /// The bridge every filtering test writes to, with both listings answered.
    fn full_bridge() -> ScriptedBridge {
        ScriptedBridge {
            rooms: Some(CLIP_ROOMS),
            lights: Some(CLIP_LIGHTS),
            gets: RefCell::new(Vec::new()),
            puts: RefCell::new(Vec::new()),
        }
    }

    /// Every path `local` wrote to, in order, for one config and one behaviour.
    ///
    /// NOON, and no fallback window: a test about skip lists must not be
    /// deciding anything with a clock.
    fn paths_written(written: &str, behaviour: Behaviour) -> Vec<String> {
        signalled(written, behaviour, Ok(None), Some(720)).0
    }

    /// Every path `local` wrote to and every refusal it answered, for one
    /// config, one behaviour, one minute of the day, and one house window that
    /// either parsed or came down as the refusal it failed with.
    fn signalled(
        written: &str,
        behaviour: Behaviour,
        fallback: Result<Option<&QuietWindow>, &str>,
        minutes_now: Option<u16>,
    ) -> (Vec<String>, Vec<String>) {
        let bridge = full_bridge();
        let lights = lights(written);
        let map = resolve_on_bridge(&bridge, &lights.families).expect("the bridge answered");
        let refusals = signal_family(
            &bridge,
            &map,
            LOCAL_FAMILY,
            &lights,
            behaviour,
            fallback,
            minutes_now,
        );
        let puts = bridge.puts.borrow();
        (
            puts.iter().map(|(path, _)| path.clone()).collect(),
            refusals,
        )
    }

    #[test]
    fn a_behaviour_a_place_skips_reaches_that_lamp_and_no_other_lamp_loses_it() {
        // COMPLETENESS OVER COUNTS: every lamp in the map is named, because a
        // count of two would pass just as well for a skip that took the wrong
        // lamp out. HCL1 refuses `done` and HCL2 refuses `breathing`, so one
        // config proves both directions at once: the skip that fires and the
        // skip that is about some other behaviour entirely.
        let config = format!(
            "{STUDIO_LOCAL}[lights.places.\"3F - Studio - HCL1\"]\nskip = [\"done\"]\n\
             [lights.places.\"3F - Studio - HCL2\"]\nskip = [\"breathing\"]\n"
        );
        assert_eq!(
            paths_written(&config, Behaviour::Done),
            vec![format!("light/{HCL2}")],
            "HCL1 skipped this behaviour and HCL2 skipped a different one"
        );
        assert_eq!(
            paths_written(&config, Behaviour::Failed),
            vec![format!("light/{HCL1}"), format!("light/{HCL2}")],
            "and a behaviour neither lamp named reaches both of them"
        );
    }

    #[test]
    fn a_lamp_inside_its_own_quiet_window_stays_dark_while_its_neighbour_signals() {
        // THE GATE IS PER FIXTURE NOW. It used to answer one question for the
        // whole pulse, so one lamp's night was every lamp's night; a bedroom
        // asleep at 23:00 must not take the studio dark with it.
        let config = format!(
            "{STUDIO_LOCAL}[lights.places.\"3F - Studio - HCL1\"]\n\
             quiet_hours = \"22:00-07:00\"\n"
        );
        assert_eq!(
            signalled(&config, Behaviour::Done, Ok(None), Some(1380)).0,
            vec![format!("light/{HCL2}")],
            "23:00: HCL1 is asleep and HCL2 never asked to be"
        );
        assert_eq!(
            signalled(&config, Behaviour::Done, Ok(None), Some(720)).0,
            vec![format!("light/{HCL1}"), format!("light/{HCL2}")],
            "noon: nobody is inside a window"
        );
    }

    #[test]
    fn the_settings_chain_answers_each_setting_on_its_own_and_the_lamp_answers_first() {
        // FOUR RUNGS, ONE SETTING AT A TIME. The lamp's own entry beats its
        // room's, and a lamp that named only a skip list has said NOTHING about
        // hours, so its room's window still applies to it. An entry-shaped
        // chain would take the room's hours away the moment the lamp wrote any
        // key at all, which is a lamp that stops sleeping because it started
        // refusing one behaviour.
        //
        // THE ROOM IS ASLEEP IN EVERY CASE and the lamp's own answer is what
        // varies, so a chain that read the room first would light nothing here.
        let room_asleep = "[lights.places.\"3F - Studio\"]\nquiet_hours = \"22:00-07:00\"\n";
        // The far side of the clock from the room's window, so a lamp that
        // takes its own hours is awake exactly when its room is not.
        let lamp_awake = "quiet_hours = \"07:00-22:00\"\n";
        let midnight = Some(15);
        // `[plugins.hue] quiet_hours`, the last rung: parsed by the composition
        // root and handed down already judged.
        let house_window = QuietWindow {
            start: 22 * 60,
            end: 7 * 60,
        };

        for (label, config, fallback, lit) in [
            (
                "the lamp's own hours beat its room's",
                format!(
                    "{STUDIO_LOCAL}{room_asleep}\
                     [lights.places.\"3F - Studio - HCL1\"]\n{lamp_awake}"
                ),
                Ok(None),
                vec![format!("light/{HCL1}")],
            ),
            (
                "a lamp that named only a skip list still inherits its room's hours",
                format!(
                    "{STUDIO_LOCAL}{room_asleep}\
                     [lights.places.\"3F - Studio - HCL1\"]\nskip = [\"breathing\"]\n"
                ),
                Ok(None),
                Vec::new(),
            ),
            (
                "neither the lamp nor its room named hours, so the house's apply",
                STUDIO_LOCAL.to_string(),
                Ok(Some(&house_window)),
                Vec::new(),
            ),
            (
                "and with nothing anywhere there is no window at all",
                STUDIO_LOCAL.to_string(),
                Ok(None),
                vec![format!("light/{HCL1}"), format!("light/{HCL2}")],
            ),
        ] {
            assert_eq!(
                signalled(&config, Behaviour::Done, fallback, midnight).0,
                lit,
                "case: {label}"
            );
        }
    }

    #[test]
    fn a_window_nobody_can_parse_darkens_that_lamp_alone_and_says_which_one() {
        // FAIL CLOSED AT THE NEW GRANULARITY. The shipped gate returns for the
        // WHOLE pulse on a refusal, which was right when one window covered
        // one house; with a window per place, one typo taking every lamp dark
        // is a house that stops signalling over a bedroom.
        //
        // AND THE REFUSAL NAMES THE PLACE, because an operator standing in a
        // dark room cannot tell a lamp that refused from a lamp that is asleep,
        // and "quiet_hours is wrong somewhere" is not something they can act on.
        let config = format!(
            "{STUDIO_LOCAL}[lights.places.\"3F - Studio - HCL1\"]\n\
             quiet_hours = \"10pm-7am\"\n"
        );
        let (written, refusals) = signalled(&config, Behaviour::Done, Ok(None), Some(720));
        assert_eq!(
            written,
            vec![format!("light/{HCL2}")],
            "the lamp with the unreadable window stays dark and its neighbour does not"
        );
        assert_eq!(
            refusals.len(),
            1,
            "one refusal, not one per lamp: {refusals:?}"
        );
        assert!(
            refusals[0].contains("3F - Studio - HCL1") && refusals[0].contains("10pm-7am"),
            "naming the place and echoing what was written: {}",
            refusals[0]
        );

        // AND IT NAMES THE ENTRY THAT WAS WRONG, not the lamp that inherited
        // it. A room's unreadable window darkens every lamp in it, and sending
        // the operator to `lights.places."3F - Studio - HCL1"` to fix a typo
        // that is in `lights.places."3F - Studio"` is a message they act on and
        // get nowhere with.
        let room_typo =
            format!("{STUDIO_LOCAL}[lights.places.\"3F - Studio\"]\nquiet_hours = \"10pm-7am\"\n");
        let (written, refusals) = signalled(&room_typo, Behaviour::Done, Ok(None), Some(720));
        assert!(
            written.is_empty(),
            "both lamps inherited the unreadable window: {written:?}"
        );
        assert_eq!(
            refusals.len(),
            1,
            "ONE typo is ONE refusal, however many lamps read it: {refusals:?}"
        );
        assert!(
            refusals[0].contains("\"3F - Studio\"") && !refusals[0].contains("HCL"),
            "the room's entry is what is wrong, so the room is what is named: {}",
            refusals[0]
        );
    }

    #[test]
    fn a_house_window_nobody_can_parse_darkens_only_the_lamps_that_reach_it() {
        // THE FALLBACK IS A RUNG, NOT A DOOR. `[plugins.hue] quiet_hours` used
        // to be judged before the map was consulted at all, so one typo in the
        // house key took every lamp dark however carefully its own place was
        // written. A lamp whose own entry states hours that parse never reads
        // the house key, and a lamp that reaches it fails closed alone.
        let house_refusal = "pns: config error (hue.quiet_hours is \"10pm-7am\", \
                             not a HH:MM-HH:MM window); no pulse";
        let config = format!(
            "{STUDIO_LOCAL}[lights.places.\"3F - Studio - HCL1\"]\n\
             quiet_hours = \"22:00-07:00\"\n"
        );
        let (written, refusals) = signalled(
            &config,
            Behaviour::Done,
            Err(house_refusal),
            // Noon, so HCL1's own window is one it is awake outside of.
            Some(720),
        );
        assert_eq!(
            written,
            vec![format!("light/{HCL1}")],
            "the lamp with its own readable window signals; the one that fell \
             through to the house key does not"
        );
        assert_eq!(
            refusals,
            vec![house_refusal.to_string()],
            "and the refusal that reaches the operator names the HOUSE key, \
             because that is the entry they have to go and fix"
        );
    }

    // --- the pre-resolution gate: could this family signal at all? ----------

    /// The house window, and it is QUIET at the minute every case below reads.
    const NIGHT: QuietWindow = QuietWindow {
        start: 22 * 60,
        end: 7 * 60,
    };

    /// Quarter past midnight: inside `NIGHT`, outside its complement.
    const MIDNIGHT: Option<u16> = Some(15);

    /// The far side of the clock from `NIGHT`, so a place that states it is
    /// awake exactly when the house is asleep.
    const DAYTIME: &str = "quiet_hours = \"07:00-22:00\"\n";

    #[test]
    fn the_gate_answers_for_the_routed_family_and_nobody_else() {
        for (label, config, fallback, loud) in [
            (
                "a place no claim reaches cannot buy the bridge a round trip",
                format!("{STUDIO_LOCAL}[lights.places.\"3F - Nowhere\"]\n{DAYTIME}"),
                Ok(Some(&NIGHT)),
                false,
            ),
            (
                "a family whose claim states its own quiet hours answers for itself, \
                 however loud the house is",
                format!(
                    "{STUDIO_LOCAL}[lights.places.\"3F - Studio\"]\nquiet_hours = \"22:00-07:00\"\n"
                ),
                Ok(None),
                false,
            ),
            (
                "and so does a claim written as a light rather than a room",
                "[lights.families.local]\nlights = [\"3F - Studio - HCL1\"]\n\
                 [lights.places.\"3F - Studio - HCL1\"]\nquiet_hours = \"22:00-07:00\"\n"
                    .to_string(),
                Ok(None),
                false,
            ),
            (
                "a claim that named no hours takes the house window",
                STUDIO_LOCAL.to_string(),
                Ok(Some(&NIGHT)),
                false,
            ),
            (
                "and with no house window there is nothing to be quiet inside",
                STUDIO_LOCAL.to_string(),
                Ok(None),
                true,
            ),
            (
                "a claim awake inside its own hours is loud whatever the house says",
                format!("{STUDIO_LOCAL}[lights.places.\"3F - Studio\"]\n{DAYTIME}"),
                Ok(Some(&NIGHT)),
                true,
            ),
            (
                "and a family holding no claims at all can signal nothing",
                "[lights.families.loop]\nrooms = [\"3F - Studio\"]\n".to_string(),
                Ok(None),
                false,
            ),
        ] {
            assert_eq!(
                any_place_loud(&lights(&config), LOCAL_FAMILY, fallback, MIDNIGHT),
                loud,
                "case: {label}"
            );
        }
    }

    #[test]
    fn a_claimed_window_nobody_can_parse_answers_loud() {
        // A CONFIG THIS CANNOT JUDGE IS NOT A CONFIG PROVEN DARK. The refusal
        // that names the entry is printed by the fixture walk, and this gate
        // has to err loud for that walk to be reached at all: reading an
        // unreadable window as ASLEEP would take every lamp dark over one typo
        // and tell the operator nothing, which is the silent mute this whole
        // table exists to refuse.
        assert!(
            any_place_loud(
                &lights(&format!(
                    "{STUDIO_LOCAL}[lights.places.\"3F - Studio\"]\n\
                     quiet_hours = \"25:99-xx\"\n"
                )),
                LOCAL_FAMILY,
                Ok(Some(&NIGHT)),
                MIDNIGHT,
            ),
            "an unreadable window on a claimed place, under a house window that \
             is quiet right now"
        );
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

    // --- the tick: one state, per fixture ------------------------------------

    /// The map this repo ships: `local` holds the studio minus HCL3, `loop`
    /// holds HCL3.
    const LOOP_MAP: &str = "[lights.families.local]\nrooms = [\"3F - Studio\"]\n\
         except = [\"3F - Studio - HCL3\"]\n\
         [lights.families.loop]\nlights = [\"3F - Studio - HCL3\"]\n";

    /// The needs-you STATE body at a 20-second refresh: the same two deep blues
    /// the pulse alternates, held for one refresh interval plus its slack so a
    /// lamp is never dark between two re-arms.
    const NEEDS_YOU_STATE: &str = r#"{"signaling":{"colors":[{"xy":{"x":0.1532,"y":0.0475}},{"xy":{"x":0.15,"y":0.06}}],"duration":25000,"signal":"alternating"}}"#;
    /// The breathe: the bridge's OWN alert action, which it renders as a
    /// smooth swell around whatever colour the lamp is already showing and
    /// ends by itself after about fifteen seconds.
    const BREATHING_ALERT: &str = r#"{"alert":{"action":"breathe"}}"#;
    /// The glow: a TRUE STEADY write, and the one body on this path that does
    /// not expire on its own.
    const GLOW_STEADY: &str =
        r#"{"color":{"xy":{"x":0.4,"y":0.19}},"dimming":{"brightness":25.0},"on":{"on":true}}"#;
    /// What clears it.
    const GLOW_CLEAR: &str = r#"{"on":{"on":false}}"#;

    fn armed(behaviour: Behaviour, refresh_secs: u64) -> Arming<'static> {
        Arming {
            behaviour,
            refresh_secs,
            fallback: Ok(None),
            minutes_now: Some(720),
            started_minutes: Some(720),
        }
    }

    fn loop_map() -> (Resolution, crate::config::Lights) {
        let lights = crate::config::parse_config(LOOP_MAP)
            .expect("the test's own config parses")
            .lights
            .expect("and carries a lights table");
        (
            resolve(CLIP_ROOMS, Some(CLIP_LIGHTS), &lights.families),
            *lights,
        )
    }

    #[test]
    fn a_state_reaches_only_the_fixtures_of_the_family_that_produces_it() {
        let (map, lights) = loop_map();
        let bridge = scripted(None);
        let written = signal_state(&bridge, &map, &lights, &armed(Behaviour::NeedsYou, 20));
        assert_eq!(
            bridge.puts.borrow().as_slice(),
            &[
                (format!("light/{HCL1}"), NEEDS_YOU_STATE.to_string()),
                (format!("light/{HCL2}"), NEEDS_YOU_STATE.to_string()),
            ],
            "the blue state is local's, so the loop lamp is dark for it: every fixture \
             in the map is named here rather than counted"
        );
        assert!(written.refusals.is_empty() && written.held.is_empty());

        let bridge = scripted(None);
        let written = signal_state(&bridge, &map, &lights, &armed(Behaviour::Breathing, 20));
        assert_eq!(
            bridge.puts.borrow().as_slice(),
            &[(format!("light/{HCL3}"), BREATHING_ALERT.to_string())],
            "and breathing is the loop lamp's, so the local pair stays dark for it"
        );
        assert!(
            written.held.is_empty(),
            "the bridge ends its own swell, so there is nothing to remember"
        );
    }

    #[test]
    fn breathing_is_the_bridges_own_breathe_and_carries_no_signal_at_all() {
        // THE OPERATOR'S DECISION OF 2026-08-30. Every shape this crate could
        // build out of `signalling` was either a strobe or a turn signal, and
        // the bridge already renders the thing that was wanted: a smooth swell
        // around the lamp's current colour, ended by the bridge itself after
        // about fifteen seconds.
        assert_eq!(
            state_body(Behaviour::Breathing, 20).as_deref(),
            Some(BREATHING_ALERT)
        );
        // NO REFRESH INTERVAL REACHES THIS BODY. How long a swell lasts is the
        // bridge's business, not ours, and a duration computed here would be a
        // number the bridge ignores.
        assert_eq!(
            state_body(Behaviour::Breathing, 900).as_deref(),
            Some(BREATHING_ALERT),
            "a fifteen-minute refresh still asks for exactly one breathe"
        );
        let body = state_body(Behaviour::Breathing, 20).expect("a breathing body");
        assert!(
            !body.contains("signaling") && !body.contains("on_off_color") && !body.contains("xy"),
            "no signal, no flash shape and no colour of our own: {body}"
        );
    }

    #[test]
    fn a_glow_is_a_steady_write_and_the_tick_clears_it_when_the_condition_goes() {
        let (map, lights) = loop_map();
        let bridge = scripted(None);
        let written = signal_state(&bridge, &map, &lights, &armed(Behaviour::Glow, 20));
        assert_eq!(
            bridge.puts.borrow().as_slice(),
            &[(format!("light/{HCL3}"), GLOW_STEADY.to_string())],
            "the one body on this path that does not expire on its own"
        );
        assert_eq!(
            written.held,
            vec![format!("light/{HCL3}")],
            "so the tick has to remember what it is holding, or nothing could put it out"
        );

        // THE SECOND OF THE TWO CLEARS the steady write is paid for with: any
        // tick that sees the condition gone puts the lamp out by name, off the
        // held paths alone and with no bridge listing to resolve them again.
        let bridge = scripted(None);
        clear_held(&bridge, &written.held);
        assert_eq!(
            bridge.puts.borrow().as_slice(),
            &[(format!("light/{HCL3}"), GLOW_CLEAR.to_string())],
        );
        assert!(
            bridge.gets.borrow().is_empty(),
            "a clear takes no GET, which is what lets the EVENT path make one with no daemon"
        );
    }

    #[test]
    fn a_lights_own_catch_up_overrides_its_rooms_and_a_silent_lamp_inherits_it() {
        // SPECIFIC FIRST, PER SETTING: the first rung that STATED `catch_up` is
        // the one that decides. Read as "any rung that set it", a lamp had no
        // spelling at all for turning its room's catch-up back off, which is the
        // one direction an operator writing `false` can only have meant.
        const NIGHT: &str = "22:00-07:00";
        const EIGHT_AM: Option<u16> = Some(8 * 60);
        const ELEVEN_PM: Option<u16> = Some(23 * 60);
        let placement = Placement {
            name: "3F - Studio - HCL3".to_string(),
            room: Some("3F - Studio".to_string()),
        };
        let shows = |written: &str| {
            let lights = crate::config::parse_config(written)
                .expect("the test's own config parses")
                .lights
                .expect("and carries a lights table");
            place_shows_state(
                &lights,
                &placement,
                Behaviour::Glow,
                Ok(None),
                EIGHT_AM,
                ELEVEN_PM,
            )
        };
        let room = format!(
            "[lights.places.\"3F - Studio\"]\nquiet_hours = \"{NIGHT}\"\ncatch_up = true\n"
        );
        assert_eq!(
            shows(&format!(
                "{room}[lights.places.\"3F - Studio - HCL3\"]\ncatch_up = false\n"
            )),
            Ok(false),
            "the lamp stated false and it is the more specific rung, so the room's \
             true loses"
        );
        assert_eq!(
            shows(&room),
            Ok(true),
            "and the room's true still reaches a lamp that said nothing about \
             catch-up: PER SETTING, so writing one key never takes the others away"
        );
        assert_eq!(
            shows(&format!(
                "{room}[lights.places.\"3F - Studio - HCL3\"]\nskip = [\"done\"]\n"
            )),
            Ok(true),
            "including a lamp that stated some OTHER setting entirely"
        );
    }

    #[test]
    fn a_state_that_began_inside_the_quiet_window_is_not_shown_after_it_without_catch_up() {
        // 22:00 to 07:00, asked at 08:00, about a state that began at 23:00.
        const NIGHT: &str = "22:00-07:00";
        const EIGHT_AM: Option<u16> = Some(8 * 60);
        const ELEVEN_PM: Option<u16> = Some(23 * 60);
        const HALF_PAST_SEVEN: Option<u16> = Some(7 * 60 + 30);
        let place = |extra: &str| {
            let lights = crate::config::parse_config(&format!(
                "[lights.places.\"3F - Studio - HCL3\"]\nquiet_hours = \"{NIGHT}\"\n{extra}"
            ))
            .expect("the test's own config parses")
            .lights
            .expect("and carries a lights table");
            *lights
        };
        let placement = Placement {
            name: "3F - Studio - HCL3".to_string(),
            room: None,
        };
        let shows = |lights: &crate::config::Lights, started| {
            place_shows_state(
                lights,
                &placement,
                Behaviour::Glow,
                Ok(None),
                EIGHT_AM,
                started,
            )
        };
        assert_eq!(
            shows(&place(""), ELEVEN_PM),
            Ok(false),
            "the operator's DEFAULT: news suppressed through the night is not news at 08:00"
        );
        assert_eq!(
            shows(&place("catch_up = true\n"), ELEVEN_PM),
            Ok(true),
            "and the opt-in is the whole difference"
        );
        assert_eq!(
            shows(&place(""), HALF_PAST_SEVEN),
            Ok(true),
            "a state that began AFTER the window ended is not a leftover of it"
        );
        assert_eq!(
            shows(&place(""), None),
            Ok(false),
            "a start this machine cannot place in the day fails toward dark"
        );

        // AND IT IS WIRED, not merely written: the same suppression through
        // the tick's own walk.
        let (map, _) = loop_map();
        let lights = crate::config::parse_config(&format!(
            "{LOOP_MAP}[lights.places.\"3F - Studio - HCL3\"]\nquiet_hours = \"{NIGHT}\"\n"
        ))
        .expect("the test's own config parses")
        .lights
        .expect("and carries a lights table");
        let bridge = scripted(None);
        signal_state(
            &bridge,
            &map,
            &lights,
            &Arming {
                behaviour: Behaviour::Glow,
                refresh_secs: 20,
                fallback: Ok(None),
                minutes_now: EIGHT_AM,
                started_minutes: ELEVEN_PM,
            },
        );
        assert!(
            bridge.puts.borrow().is_empty(),
            "the lamp that slept through the news stays dark at 08:00"
        );
    }
}
