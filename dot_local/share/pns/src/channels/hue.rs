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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuietWindow {
    start: u16,
    end: u16,
}

impl QuietWindow {
    /// The minute of the local day this window ends at, which is the one thing
    /// a BARE `pns lights quiet` needs from it.
    pub fn ends_at(&self) -> u16 {
        self.end
    }
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
/// room and is all the room-shaped `[plugins.hue] rooms` pulse ever needs; a
/// light is what the ROUTED path always resolves to, because arbitration, the
/// dim window and the mute are each per lamp, and a group write would reach a
/// lamp that answered any of the three differently.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Fixture {
    Grouped(String),
    Light(String),
}

impl Fixture {
    /// The CLIP resource path this fixture is written to.
    ///
    /// WHICH IS THE WHOLE POINT OF THE DISTINCTION. Addressing either as the
    /// other is a PUT to a resource id of the wrong type, which the bridge
    /// answers by doing nothing and telling no one, because `put` is fire and
    /// forget.
    pub fn path(&self) -> String {
        match self {
            Fixture::Grouped(id) => format!("grouped_light/{id}"),
            Fixture::Light(id) => format!("light/{id}"),
        }
    }
}

/// A declaration that answered no lamp, and which level wrote it.
///
/// REPORTED, NEVER DROPPED. `grouped_light_ids_for_rooms` drops a missing room
/// in silence, which is survivable for a two-room pulse an operator can see
/// failing. A routing map is large enough that a typo is a lamp which never
/// lights, and a silent drop makes it unfalsifiable.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Unresolved {
    pub level: String,
    pub name: String,
    pub kind: Missing,
}

/// WHY a declaration answered no lamp, because the two are different problems
/// and an operator fixes them in different places.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Missing {
    /// No lamp, room or zone of that name in the bridge's listings. A spelling
    /// to fix, or a lamp to plug in.
    NotOnBridge,
    /// The name IS on the bridge and it holds no lamp: an empty room, or a zone
    /// whose members are all switches. A map to fix, and telling an operator to
    /// go looking for a room sitting right there in the bridge would be a lie
    /// they act on.
    AddressedNothing,
}

/// What is wrong with one declaration, in one sentence and with no prefix on it.
///
/// ONE WORDING, TWO READERS. The doctor prefixes it with its own name and the
/// tick with its own, and an operator who reads the same lamp reported two
/// different ways has to work out whether they are the same problem.
pub fn missing_sentence(missing: &Unresolved) -> String {
    match missing.kind {
        Missing::NotOnBridge => format!(
            "lights: `{}` ({}) is not on the bridge",
            missing.name, missing.level
        ),
        Missing::AddressedNothing => format!(
            "lights: `{}` ({}) is on the bridge, but it holds no lamp",
            missing.name, missing.level
        ),
    }
}

/// One lamp on the bridge, with every name a declaration can reach it by.
///
/// THREE NAMES BECAUSE THERE ARE THREE LEVELS. Resolution walks them
/// specific-first, so a lamp that knew only its own name would make
/// "specific beats general" unstatable for every question it did not answer
/// itself.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Lamp {
    pub id: String,
    pub name: String,
    pub room: Option<String>,
    /// EVERY zone holding it, not one. A lamp belongs to exactly one room and
    /// to any number of zones, which is the whole reason the double-cover
    /// refusal exists at the zone level and nowhere else.
    pub zones: Vec<String>,
}

/// What the bridge holds: its lamps, and the room and zone names it knows.
///
/// THE NAME LISTS ARE WHAT TELL A TYPO FROM AN EMPTY ROOM. Without them a room
/// that exists and holds no lamp is indistinguishable from one the operator
/// misspelled, and those are different sentences an operator acts on
/// differently.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Inventory {
    pub lamps: Vec<Lamp>,
    pub rooms: Vec<String>,
    pub zones: Vec<String>,
}

/// One lamp in the bridge's light listing, before the two joins run.
struct RawLamp {
    id: String,
    name: String,
    /// The DEVICE that owns it, which is what a ROOM lists as its child. The
    /// room join runs through here; the zone join does not.
    owner: String,
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

/// One listing entry's `metadata.name`.
fn named(entry: &serde_json::Value) -> Option<String> {
    Some(entry.pointer("/metadata/name")?.as_str()?.to_string())
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

/// The window a lamp runs dim inside, and which behaviours run dim there.
///
/// THE ENABLES RIDE THE WINDOW, which is what makes them one question: a
/// declaration either states when the lamp is quiet and what it does then, or
/// it says nothing about quiet hours at all. Two separately inherited halves
/// would let a lamp take its room's window and a zone's enables.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DimWindow {
    pub window: QuietWindow,
    /// EMPTY IS SUPPRESS EVERYTHING, and it needs no second mode to spell: a
    /// window with nothing enabled takes every behaviour away for its duration,
    /// which is the bedroom rule with no special case in the code.
    pub behaviours: Vec<crate::config::Behaviour>,
}

/// One lamp with every question answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Routed {
    pub lamp: Lamp,
    pub shows: Vec<crate::config::Behaviour>,
    pub dim: Option<DimWindow>,
}

/// Every lamp any declaration reaches, plus what could not be resolved and what
/// was refused.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Routing {
    /// ONLY LAMPS THAT CARRY SOMETHING. A lamp resolved to an empty `shows`
    /// list carries nothing, which is what a deliberate empty declaration means
    /// as much as what silence means, so both leave the lamp out of the walks
    /// rather than costing a write that does nothing.
    pub lamps: Vec<Routed>,
    pub unresolved: Vec<Unresolved>,
    /// Declarations this refused outright: a lamp two zones both answer for,
    /// and a window nobody can parse. Deduplicated, in the order they were met.
    pub refusals: Vec<String>,
}

/// The three levels, most specific first. THE ORDER IS THE PRECEDENCE, so the
/// walk is a `find` over this rather than a chain of `if`s per question.
const LEVELS: [&str; 3] = ["lamp", "room", "zone"];

/// Names to lamps: pure, total, and loud about what it could not resolve.
///
/// EVERY QUESTION RESOLVES ON ITS OWN. A lamp's own declaration can state which
/// behaviours it carries and say nothing about quiet hours, and its room's
/// window still applies; an entry-shaped chain would have taken that away the
/// moment the lamp wrote one key. The rule is the same for both questions and
/// it is applied twice rather than written twice.
///
/// THE WINNING LEVEL SUPPLIES THE WHOLE ANSWER TO ITS QUESTION, and levels never
/// merge. Merging was rejected because a room's lamps have to be able to differ:
/// a union would re-add exactly what a lamp-level declaration deliberately left
/// out, and the operator's own routing needs one lamp in a room to carry the
/// held states while the rest carry the pulses.
///
/// TWO ZONES ANSWERING ONE QUESTION FOR ONE LAMP IS REFUSED, with both named.
/// There is no specificity between them to arbitrate and guessing is against
/// house style, so that question answers NOTHING for that lamp and the operator
/// is told which two declarations to break the tie between. The other levels
/// cannot collide: a lamp has one name and one room, and TOML refuses a
/// duplicated table key itself.
///
/// THE BRIDGE'S CURRENT MEMBERSHIP IS THE TRUTH. A lamp named by room A's
/// declaration and physically moved to room B answers room B's, because the
/// join is taken from the listing this call was handed rather than from
/// anything remembered.
pub fn resolve(inventory: &Inventory, lights: &crate::config::Lights) -> Routing {
    let mut routing = Routing {
        unresolved: unresolved_names(inventory, lights),
        ..Routing::default()
    };
    for lamp in &inventory.lamps {
        let shows = winner(&mut routing, lamp, lights, "shows", |target| {
            target.shows.clone()
        })
        .unwrap_or_default();
        let dim = winner(&mut routing, lamp, lights, "dim_window", |target| {
            target
                .dim_window
                .as_ref()
                .map(|stated| (stated.clone(), target.dim_behaviours.clone()))
        });
        let dim = match dim {
            None => None,
            Some((stated, behaviours)) => match parse_window(&stated) {
                Some(window) => Some(DimWindow { window, behaviours }),
                // FAIL CLOSED, FOR THIS LAMP ALONE. An operator who asked for a
                // dim window and mistyped it would otherwise be flashed at 3am
                // and told nothing; what the refusal buys is that the cost is
                // one lamp rather than the whole house.
                None => {
                    remember(&mut routing.refusals, window_refusal(&lamp.name, &stated));
                    continue;
                }
            },
        };
        if shows.is_empty() {
            continue;
        }
        routing.lamps.push(Routed {
            lamp: lamp.clone(),
            shows,
            dim,
        });
    }
    routing
}

/// The most specific declaration that STATES one question, or a refusal when
/// two zones both do.
fn winner<Answer>(
    routing: &mut Routing,
    lamp: &Lamp,
    lights: &crate::config::Lights,
    question: &str,
    stated: impl Fn(&crate::config::Target) -> Option<Answer>,
) -> Option<Answer> {
    for level in LEVELS {
        let answers: Vec<(&String, Answer)> = declarations(lamp, lights, level)
            .filter_map(|(name, target)| Some((name, stated(target)?)))
            .collect();
        match answers.len() {
            0 => continue,
            1 => return answers.into_iter().next().map(|(_, answer)| answer),
            _ => {
                let names: Vec<String> = answers
                    .iter()
                    .map(|(name, _)| format!("{name:?}"))
                    .collect();
                remember(
                    &mut routing.refusals,
                    double_cover_refusal(&lamp.name, level, question, &names),
                );
                return None;
            }
        }
    }
    None
}

/// Every declaration at one level that names this lamp.
fn declarations<'settings>(
    lamp: &'settings Lamp,
    lights: &'settings crate::config::Lights,
    level: &str,
) -> impl Iterator<Item = (&'settings String, &'settings crate::config::Target)> {
    let (table, names): (
        &std::collections::BTreeMap<String, crate::config::Target>,
        Vec<&str>,
    ) = match level {
        "lamp" => (&lights.lamps, vec![lamp.name.as_str()]),
        "room" => (&lights.rooms, lamp.room.as_deref().into_iter().collect()),
        _ => (
            &lights.zones,
            lamp.zones.iter().map(String::as_str).collect(),
        ),
    };
    table
        .iter()
        .filter(move |(name, _)| names.contains(&name.as_str()))
}

/// Every declared name the bridge could not answer, and why.
fn unresolved_names(inventory: &Inventory, lights: &crate::config::Lights) -> Vec<Unresolved> {
    let mut missing = Vec::new();
    let holds = |level: &str, name: &str| {
        inventory.lamps.iter().any(|lamp| match level {
            "lamp" => lamp.name == name,
            "room" => lamp.room.as_deref() == Some(name),
            _ => lamp.zones.iter().any(|zone| zone == name),
        })
    };
    for (level, declared, known) in [
        ("lamp", &lights.lamps, None),
        ("room", &lights.rooms, Some(&inventory.rooms)),
        ("zone", &lights.zones, Some(&inventory.zones)),
    ] {
        for name in declared.keys() {
            if holds(level, name) {
                continue;
            }
            // ON THE BRIDGE AND EMPTY IS NOT THE SAME AS ABSENT. A lamp level
            // has no third state: a name that reaches no lamp IS the lamp that
            // is not there.
            let kind = match known {
                Some(names) if names.iter().any(|known| known == name) => Missing::AddressedNothing,
                _ => Missing::NotOnBridge,
            };
            missing.push(Unresolved {
                level: level.to_string(),
                name: name.clone(),
                kind,
            });
        }
    }
    missing.sort();
    missing
}

/// ONE REFUSAL PER PROBLEM, not per lamp that met it: two lamps inheriting one
/// room's unreadable window is one typo, and saying it twice trains an operator
/// to skim the line.
fn remember(refusals: &mut Vec<String>, refusal: String) {
    if !refusals.contains(&refusal) {
        refusals.push(refusal);
    }
}

fn double_cover_refusal(lamp: &str, level: &str, question: &str, names: &[String]) -> String {
    format!(
        "lights: `{lamp}` is covered by {} {level} declarations that each state \
         `{question}` ({}); there is nothing more specific to break the tie, so \
         that lamp answers none of them",
        names.len(),
        names.join(" and ")
    )
}

fn window_refusal(lamp: &str, stated: &str) -> String {
    format!(
        "lights: `{lamp}` has dim_window {stated:?}, which is not a HH:MM-HH:MM \
         window; that lamp stays dark"
    )
}

/// What a lamp does with one behaviour right now.
///
/// THREE ANSWERS RATHER THAN A BOOLEAN, because a dim window no longer means one
/// thing: inside it a behaviour either runs its dim form or is taken away
/// entirely, and the caller has to know which body to write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Showing {
    Dark,
    Full,
    Dimmed,
}

/// Which of the three a lamp shows, given the minute of the local day.
///
/// PER BEHAVIOUR, WHICH IS THE WHOLE DESIGN. Inside the window a behaviour the
/// operator enabled runs its dim form and one they did not is suppressed, so a
/// room can breathe faintly about a wait all night while refusing to strobe
/// green about a build. A window with nothing enabled suppresses everything,
/// which is the bedroom rule and needs no mode of its own.
///
/// A LAMP WITH NO WINDOW IS UNTOUCHED. That is what makes the whole feature
/// opt-in: a config that never states a window pays nothing and behaves exactly
/// as it did.
///
/// AN UNREADABLE CLOCK IS INSIDE THE WINDOW, through `quiet_now`'s own rule: a
/// flash at 3am is what the window was set to prevent, and a missed signal
/// costs nothing.
pub fn dim_showing(
    dim: Option<&DimWindow>,
    behaviour: crate::config::Behaviour,
    minutes_now: Option<u16>,
) -> Showing {
    let Some(dim) = dim else {
        return Showing::Full;
    };
    if !quiet_now(Some(&dim.window), minutes_now) {
        return Showing::Full;
    }
    if dim.behaviours.contains(&behaviour) {
        Showing::Dimmed
    } else {
        Showing::Dark
    }
}

/// Whether the operator's own by-hand mute covers this lamp.
///
/// EVERY NAME THE LAMP ANSWERS TO, which is the same vocabulary a declaration
/// names it by: `pns lights quiet "3F - Studio"` reaches every lamp in the
/// studio and `pns lights quiet "3F - Studio - HCL3"` reaches one. A zone name
/// works for the same reason.
pub fn muted_now(lamp: &Lamp, muted: &[String]) -> bool {
    std::iter::once(lamp.name.as_str())
        .chain(lamp.room.as_deref())
        .chain(lamp.zones.iter().map(String::as_str))
        .any(|name| muted.iter().any(|quiet| quiet == name))
}

/// Every name a declaration writes, at any level, sorted and deduplicated.
///
/// THE VOCABULARY `pns lights quiet` ACCEPTS, and it is the config's own rather
/// than the bridge's on purpose: this answer is wanted by a typed command that
/// must not dial a bridge to refuse a typo, and a name the config holds but the
/// bridge does not is already reported by the tick and the doctor in their own
/// words.
pub fn declared_names(lights: &crate::config::Lights) -> Vec<String> {
    let mut names: Vec<String> = lights
        .lamps
        .keys()
        .chain(lights.rooms.keys())
        .chain(lights.zones.keys())
        .cloned()
        .collect();
    names.sort();
    names.dedup();
    names
}

/// The bridge seam: authenticated GETs and PUTs against the CLIP paths.
pub trait Bridge {
    fn get(&self, path: &str) -> Option<String>;
    /// Fire and forget: `run` discards every outcome, so a bridge that
    /// refuses tells no one. Returning a result would be a seam with no
    /// consumer.
    fn put(&self, path: &str, body: &str);
}

/// The three listings the routing is resolved from.
///
/// ALL THREE OR NOTHING. A listing that failed and a listing that was empty are
/// different answers, and collapsing them would resolve a config against an
/// empty inventory: every name it holds reported as a typo, every lamp dark,
/// all of it stated confidently about a bridge that said nothing.
pub fn resolve_on_bridge<B: Bridge>(bridge: &B, lights: &crate::config::Lights) -> Option<Routing> {
    let rooms = bridge.get("room")?;
    let lamps = bridge.get("light")?;
    let zones = bridge.get("zone")?;
    Some(resolve(&inventory(&rooms, &lamps, &zones), lights))
}

/// The brightness a body that states one runs at, as the bridge takes it.
fn dimming(percent: u8) -> serde_json::Value {
    serde_json::json!({"brightness": f64::from(percent)})
}

/// The PULSE body: a timed signal the BRIDGE runs and ends by itself.
///
/// THE BRIDGE OWNS THE WHOLE EFFECT. It flashes the colour for the duration and
/// then puts the lamp back exactly as it was, with no snapshot, no restore
/// writes and no choreography from us, which is why this channel is one PUT.
/// MEASURED ON 2026-09-01, on a real lamp, in both directions: a full state read
/// before and after a signal came back byte-identical with the lamp on and with
/// it off.
///
/// IT ALWAYS STATES A BRIGHTNESS, and that is the price of a config that can
/// dim: a `dimming` written beside a signal PERSISTS after the signal ends
/// (drill D4, 2026-08-30), so a body that said nothing would inherit whatever
/// the last dim write left. The `[plugins.hue] rooms` path below states none and
/// stays byte-identical, because nothing on that path can ever write a floor.
pub fn pulse_body(
    pulse: &crate::config::Pulse,
    color: crate::pulse::PulseColor,
    brightness: u8,
) -> String {
    serde_json::json!({
        "signaling": {
            "signal": "on_off_color",
            "duration": pulse.duration_ms,
            "colors": [{"xy": {"x": color.x, "y": color.y}}],
        },
        "dimming": dimming(brightness),
    })
    .to_string()
}

/// The body that ARMS a breath: the colour, the lamp on, and the first fade all
/// in one write.
///
/// ONE WRITE RATHER THAN TWO, because a colour write followed by a fade is a
/// visible jump: the lamp would land at whatever brightness it was already at,
/// in the new colour, before starting to move. Stating the first fade's target
/// here means the descent begins from wherever the lamp is, which is the
/// seamless join between two ticks.
pub fn breath_arm_body(
    color: crate::pulse::PulseColor,
    fade: &crate::lights::Fade,
    duration_ms: u64,
) -> String {
    serde_json::json!({
        "on": {"on": true},
        "color": {"xy": {"x": color.x, "y": color.y}},
        "dimming": dimming(fade.brightness),
        "dynamics": {"duration": duration_ms},
    })
    .to_string()
}

/// Every fade after the first: brightness and how long to take getting there,
/// and nothing else.
///
/// NO COLOUR AND NO `on`. The arm already stated both, and repeating them would
/// be two more fields the bridge has to reconcile mid-transition on every fade
/// of every breath.
pub fn fade_body(fade: &crate::lights::Fade, duration_ms: u64) -> String {
    serde_json::json!({
        "dimming": dimming(fade.brightness),
        "dynamics": {"duration": duration_ms},
    })
    .to_string()
}

/// What puts a held lamp out.
///
/// OFF, AND NOT A RESTORE. Nothing snapshotted what the lamp was doing before
/// the breath took it, and a grouped_light GET carries no colour at all, so
/// there is nothing honest to put back. Dark is what "the state is over" means
/// everywhere else on this path, and the operator's own ruling is that pns
/// animates in-use lamps.
pub fn clear_body() -> String {
    serde_json::json!({"on": {"on": false}}).to_string()
}

/// Put out every lamp a held write is still holding.
///
/// OFF THE HELD PATHS ALONE, with no listing resolved: the paths were recorded
/// when they were written, so a clear costs no GET and cannot be defeated by a
/// bridge that has stopped answering its listings. That is what lets the EVENT
/// path make this call with no daemon involved at all.
pub fn clear_held<B: Bridge>(bridge: &B, held: &[String]) {
    let body = clear_body();
    for path in held {
        bridge.put(path, &body);
    }
}

/// The colour and the breath one held state runs at, dim form or full.
///
/// THE ONE MAPPING from a state to what it looks like, read by the tick and by
/// nothing else. Its two halves travel together because a dim breath in a full
/// colour, or the reverse, is a lamp saying half of one thing.
pub fn held_render(
    held: crate::lights::Held,
    lights: &crate::config::Lights,
    showing: Showing,
) -> (crate::pulse::PulseColor, crate::config::Breath) {
    let (color, breath) = match held {
        crate::lights::Held::Blocked => (crate::pulse::BLOCKED_COLOR, lights.blocked),
        crate::lights::Held::Looping => (crate::pulse::LOOP_COLOR, lights.looping.breath),
        crate::lights::Held::UnreadFailure => (crate::pulse::FAILURE_COLOR, lights.unread.breath),
        crate::lights::Held::UnreadSuccess => {
            (crate::pulse::UNREAD_SUCCESS_COLOR, lights.unread.breath)
        }
    };
    // THE DIM FORM IS ONE SHAPE FOR EVERY BEHAVIOUR, which is what the operator
    // locked: the colour still says which state it is, and the shape says the
    // house is asleep.
    match showing {
        Showing::Dimmed => (color, lights.dim),
        Showing::Dark | Showing::Full => (color, breath),
    }
}

/// The colour and brightness one pulse fires at.
pub fn pulse_render(
    behaviour: crate::config::Behaviour,
    lights: &crate::config::Lights,
    showing: Showing,
) -> Option<(crate::pulse::PulseColor, crate::config::Pulse, u8)> {
    let (color, pulse) = match behaviour {
        crate::config::Behaviour::Done => (crate::pulse::SUCCESS_COLOR, lights.done),
        crate::config::Behaviour::Failed => (crate::pulse::FAILURE_COLOR, lights.failed),
        // A HELD STATE IS NOT A PULSE, and there is no nearest shape to fall
        // back to: a lamp asked to flash a state it holds would be armed with
        // something nobody measured.
        _ => return None,
    };
    // A DIMMED PULSE IS THE SAME BLINK AT THE DIM FLOOR, which is the faintest
    // the hardware goes; there is no low end for a blink to fade to.
    match showing {
        Showing::Dark => None,
        Showing::Full => Some((color, pulse, pulse.brightness)),
        Showing::Dimmed => Some((color, pulse, lights.dim.low)),
    }
}

/// What one lamp is judged against: the minute it is being asked about, and the
/// names the operator's own mute is covering.
pub struct Reading<'reading> {
    pub minutes_now: Option<u16>,
    /// EMPTY IS THE ORDINARY CASE, and a machine that has never run
    /// `pns lights quiet` reads an absent file as exactly that.
    pub muted: &'reading [String],
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
    /// shape both likely misconfigurations take.
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
/// AND IT STATES NO BRIGHTNESS, ever. This is the path of a machine with no
/// `[lights]` table and of `pns pulse` on a machine with one: no routing is in
/// reach to dim, so nothing here can have left a floor on a lamp and the body is
/// the one that shipped, byte for byte.
pub fn signal_fixtures<B: Bridge>(
    bridge: &B,
    fixtures: &[Fixture],
    behaviour: crate::config::Behaviour,
) -> usize {
    let (signal, color) = match behaviour {
        crate::config::Behaviour::Done => ("on_off_color", crate::pulse::SUCCESS_COLOR),
        crate::config::Behaviour::Failed => ("on_off_color", crate::pulse::FAILURE_COLOR),
        _ => return 0,
    };
    let body = serde_json::json!({
        "signaling": {
            "signal": signal,
            "duration": UNMAPPED_SIGNAL_DURATION_MS,
            "colors": [{"xy": {"x": color.x, "y": color.y}}],
        },
    })
    .to_string();
    for fixture in fixtures {
        bridge.put(&fixture.path(), &body);
    }
    fixtures.len()
}

/// How long the no-map pulse flashes, in milliseconds.
///
/// THREE SECONDS, AND IT IS NOT THE LOCKED FOUR. This is the body a machine with
/// no `[lights]` table sends, and it is kept byte-identical to what shipped: the
/// four-second figure was locked on the ROUTED path, where a per-behaviour knob
/// states it, and moving this one would change what an unconfigured machine does
/// without anybody asking for it.
const UNMAPPED_SIGNAL_DURATION_MS: u64 = 3000;

#[cfg(test)]
mod tests {
    use super::{
        Bridge, DEFAULT_ROOMS, DimWindow, HuePulse, Inventory, Missing, QuietWindow, Routing,
        Showing, Unresolved, breath_arm_body, clear_body, clear_held, declared_names, dim_showing,
        fade_body, grouped_light_ids_for_rooms, held_render, hue_settings, inventory, muted_now,
        parse_window, pulse_body, pulse_render, quiet_now, quiet_window, resolve,
        resolve_on_bridge,
    };
    use crate::config::Behaviour;
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
        /// The room listing, or None for a bridge that answered nothing: an
        /// unreachable address, a key it refuses, a body that never arrived.
        rooms: Option<&'static str>,
        /// The light listing, answered ONLY to a caller that asked for it, so a
        /// test can see which GETs were really taken.
        lights: Option<&'static str>,
        zones: Option<&'static str>,
        gets: RefCell<Vec<String>>,
        puts: RefCell<Vec<(String, String)>>,
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

    fn scripted(rooms: Option<&'static str>) -> ScriptedBridge {
        ScriptedBridge {
            rooms,
            lights: None,
            zones: None,
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
    const GREEN_SIGNAL: &str = r#"{"signaling":{"colors":[{"xy":{"x":0.17,"y":0.7}}],"duration":3000,"signal":"on_off_color"}}"#;

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
            "the light inventory is never fetched on the no-map path"
        );
    }

    #[test]
    fn the_no_map_body_states_no_brightness_and_keeps_its_own_duration() {
        // THE COMPATIBILITY CLAIM, pinned. This is the request a machine with no
        // `[lights]` table sends, and it must not gain a `dimming` field: there
        // is no routing in reach to dim, so a brightness stated here would take
        // a level the operator set by hand and hold the room at it for good.
        // Its three-second duration is deliberately NOT the routed path's
        // locked four: that figure was locked where a knob states it.
        let hue = pulse();
        hue.run(Behaviour::Done);
        let puts = hue.bridge.puts.borrow();
        assert_eq!(puts.len(), 2);
        assert_eq!(puts[0].1, GREEN_SIGNAL);
        assert!(!puts[0].1.contains("dimming"));
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
    fn a_held_state_has_no_room_shaped_body_so_the_no_map_pulse_writes_nothing() {
        // A BEHAVIOUR WITH NO PULSE SHAPE WRITES NOTHING AT ALL rather than
        // falling back to one that has one. A lamp asked to breathe would
        // otherwise flash whatever shape was nearest, which is the lying lamp
        // this whole design exists to prevent.
        for held in [Behaviour::Blocked, Behaviour::Unread, Behaviour::Looping] {
            let hue = pulse();
            assert_eq!(hue.run(held), 0, "{held:?} has no room-shaped body");
            assert!(hue.bridge.puts.borrow().is_empty());
        }
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

    // --- the inventory and the routing grammar -------------------------------

    // LIFTED FROM THE LIVE CAPTURE of 2026-08-20, trimmed to the rooms this
    // repo's own map uses, with a zone added over two of the studio lamps.
    // Every id, name and owner below the zone is the bridge's own.
    // NO TEST HERE MAKES A CALL: the listings are literals.
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

    /// A ZONE LISTS ITS LIGHTS DIRECTLY, which is the join that differs from a
    /// room's: a room's children are DEVICE rids and a zone's are LIGHT rids.
    /// `Upstairs` and `Desk` deliberately overlap on HCL1, which is what makes
    /// the same-level double cover reachable.
    const CLIP_ZONES: &str = r#"{"data":[
      {"id":"zone-1","type":"zone","metadata":{"name":"Upstairs"},
       "children":[{"rid":"17295316-360e-4259-b8fd-928caf1f9c3e","rtype":"light"},
                   {"rid":"de7b7231-1302-48ed-b0b5-9dd94763d350","rtype":"light"}]},
      {"id":"zone-2","type":"zone","metadata":{"name":"Desk"},
       "children":[{"rid":"17295316-360e-4259-b8fd-928caf1f9c3e","rtype":"light"}]},
      {"id":"zone-3","type":"zone","metadata":{"name":"Outdoors"},"children":[]}
    ]}"#;

    const HCL1: &str = "17295316-360e-4259-b8fd-928caf1f9c3e";
    #[allow(dead_code)]
    const HCL2: &str = "de7b7231-1302-48ed-b0b5-9dd94763d350";
    #[allow(dead_code)]
    const HCL3: &str = "9d52d98c-76f0-47d8-a718-4b88cd123665";
    #[allow(dead_code)]
    const KITCHEN_HCD3: &str = "0e7a5054-3720-4580-9d8e-8070216e9bfa";

    fn stock() -> Inventory {
        inventory(CLIP_ROOMS, CLIP_LIGHTS, CLIP_ZONES)
    }

    /// One config's `[lights]` table, written the way the parser answers it, so
    /// a test states a config rather than a struct literal.
    fn lights(written: &str) -> crate::config::Lights {
        *crate::config::parse_config(written)
            .expect("the test's own config parses")
            .lights
            .expect("and carries a lights table")
    }

    /// What one lamp ended up carrying, by name, so an assertion reads as the
    /// operator's own vocabulary rather than as bridge ids.
    fn carried(routing: &Routing, name: &str) -> Option<Vec<Behaviour>> {
        routing
            .lamps
            .iter()
            .find(|routed| routed.lamp.name == name)
            .map(|routed| routed.shows.clone())
    }

    #[test]
    fn a_lamp_knows_its_own_name_its_room_and_every_zone_holding_it() {
        let held = stock();
        let hcl1 = held
            .lamps
            .iter()
            .find(|lamp| lamp.name == "3F - Studio - HCL1")
            .expect("HCL1 is in the listing");
        assert_eq!(hcl1.id, HCL1);
        assert_eq!(
            hcl1.room.as_deref(),
            Some("3F - Studio"),
            "the ROOM join runs through the lamp's owning device"
        );
        assert_eq!(
            hcl1.zones,
            vec!["Upstairs".to_string(), "Desk".to_string()],
            "and the ZONE join reads the lights a zone lists DIRECTLY, which is a \
             different shape: one join for both would leave every zone empty"
        );
        let kitchen = held
            .lamps
            .iter()
            .find(|lamp| lamp.name == "2F - Kitchen - HCD3")
            .expect("the kitchen lamp is in the listing");
        assert_eq!(kitchen.room.as_deref(), Some("2F - Kitchen"));
        assert!(
            kitchen.zones.is_empty(),
            "a lamp no zone holds carries no zone name"
        );
        assert_eq!(held.lamps.len(), 4, "the dimmer switch owns no light");
        assert!(held.rooms.contains(&"3F - Cupboard".to_string()));
        assert!(held.zones.contains(&"Outdoors".to_string()));
    }

    #[test]
    fn the_most_specific_declaration_that_names_a_lamp_supplies_its_whole_behaviour_set() {
        // THE OPERATOR'S OWN ROUTING, in miniature: the room carries the pulses
        // and one lamp inside it is lifted out for the held states. A UNION
        // would re-add exactly what the lamp-level declaration left out, which
        // is why levels never merge.
        let routing = resolve(
            &stock(),
            &lights(
                "[lights.room.\"3F - Studio\"]\nshows = [\"done\", \"failed\"]\n\
                 [lights.lamp.\"3F - Studio - HCL3\"]\nshows = [\"blocked\", \"unread\"]\n",
            ),
        );
        assert_eq!(
            carried(&routing, "3F - Studio - HCL3"),
            Some(vec![Behaviour::Blocked, Behaviour::Unread]),
            "the lamp's own declaration wins outright and the room's does not merge in"
        );
        assert_eq!(
            carried(&routing, "3F - Studio - HCL1"),
            Some(vec![Behaviour::Done, Behaviour::Failed]),
            "and every other lamp in the room still takes the room's"
        );
        assert_eq!(
            carried(&routing, "2F - Kitchen - HCD3"),
            None,
            "a lamp no declaration names carries nothing"
        );
    }

    #[test]
    fn a_room_beats_a_zone_and_a_zone_answers_a_lamp_no_nearer_declaration_names() {
        let routing = resolve(
            &stock(),
            &lights(
                "[lights.zone.Upstairs]\nshows = [\"loop\"]\n\
                 [lights.room.\"3F - Studio\"]\nshows = [\"done\"]\n",
            ),
        );
        assert_eq!(
            carried(&routing, "3F - Studio - HCL1"),
            Some(vec![Behaviour::Done]),
            "a room is more specific than a zone, so the zone never answers here"
        );
        let zone_only = resolve(
            &stock(),
            &lights("[lights.zone.Upstairs]\nshows = [\"loop\"]\n"),
        );
        assert_eq!(
            carried(&zone_only, "3F - Studio - HCL1"),
            Some(vec![Behaviour::Looping]),
            "with nothing nearer, the zone is what answers"
        );
        assert_eq!(
            carried(&zone_only, "3F - Studio - HCL3"),
            None,
            "and a lamp that zone does not hold is untouched by it"
        );
    }

    #[test]
    fn a_lamp_two_zones_both_answer_for_is_refused_with_both_named() {
        // THERE IS NO SPECIFICITY TO ARBITRATE between two zones, and guessing
        // is against house style, so the question answers NOTHING for that lamp
        // and the operator is told which two declarations to break the tie
        // between.
        let routing = resolve(
            &stock(),
            &lights(
                "[lights.zone.Upstairs]\nshows = [\"loop\"]\n\
                 [lights.zone.Desk]\nshows = [\"blocked\"]\n",
            ),
        );
        assert_eq!(
            routing.refusals,
            vec![
                "lights: `3F - Studio - HCL1` is covered by 2 zone declarations that \
                 each state `shows` (\"Desk\" and \"Upstairs\"); there is nothing more \
                 specific to break the tie, so that lamp answers none of them"
                    .to_string()
            ],
            "both declarations are cited by name"
        );
        assert_eq!(
            carried(&routing, "3F - Studio - HCL1"),
            None,
            "and the contested lamp carries nothing rather than one of the two"
        );
        assert_eq!(
            carried(&routing, "3F - Studio - HCL2"),
            Some(vec![Behaviour::Looping]),
            "a lamp only ONE of them holds is unaffected: the refusal is per lamp"
        );
    }

    #[test]
    fn each_question_resolves_on_its_own_so_a_lamp_can_state_one_and_inherit_the_other() {
        // PER QUESTION, NOT WHOLESALE. A lamp that says which behaviours it
        // carries and nothing about quiet hours still takes its room's window;
        // an entry-shaped chain would have taken that away the moment the lamp
        // wrote one key.
        let routing = resolve(
            &stock(),
            &lights(
                "[lights.room.\"3F - Studio\"]\nshows = [\"done\"]\n\
                 dim_window = \"22:00-07:00\"\ndim_behaviours = [\"blocked\"]\n\
                 [lights.lamp.\"3F - Studio - HCL3\"]\nshows = [\"blocked\"]\n",
            ),
        );
        let hcl3 = routing
            .lamps
            .iter()
            .find(|routed| routed.lamp.name == "3F - Studio - HCL3")
            .expect("HCL3 is routed");
        assert_eq!(hcl3.shows, vec![Behaviour::Blocked], "its own answer");
        assert_eq!(
            hcl3.dim.as_ref().map(|dim| dim.behaviours.clone()),
            Some(vec![Behaviour::Blocked]),
            "and its room's window, because the lamp said nothing about quiet hours"
        );
    }

    #[test]
    fn an_empty_behaviour_set_leaves_a_lamp_out_rather_than_writing_to_it() {
        // A DELIBERATE EMPTY DECLARATION IS AN OVERRIDE, and it has to beat the
        // room: it is how one lamp in a routed room is taken out of service.
        let routing = resolve(
            &stock(),
            &lights(
                "[lights.room.\"3F - Studio\"]\nshows = [\"done\"]\n\
                 [lights.lamp.\"3F - Studio - HCL3\"]\nshows = []\n",
            ),
        );
        assert_eq!(carried(&routing, "3F - Studio - HCL3"), None);
        assert_eq!(routing.lamps.len(), 2, "the room's other two lamps remain");
    }

    #[test]
    fn a_name_the_bridge_does_not_have_is_reported_with_the_level_that_wrote_it() {
        let routing = resolve(
            &stock(),
            &lights(
                "[lights.lamp.\"3F - Studio - HCL9\"]\nshows = [\"done\"]\n\
                 [lights.room.\"3F - Attic\"]\nshows = [\"done\"]\n\
                 [lights.room.\"3F - Cupboard\"]\nshows = [\"done\"]\n\
                 [lights.zone.Outdoors]\nshows = [\"done\"]\n",
            ),
        );
        assert_eq!(
            routing.unresolved,
            vec![
                Unresolved {
                    level: "lamp".to_string(),
                    name: "3F - Studio - HCL9".to_string(),
                    kind: Missing::NotOnBridge,
                },
                Unresolved {
                    level: "room".to_string(),
                    name: "3F - Attic".to_string(),
                    kind: Missing::NotOnBridge,
                },
                Unresolved {
                    level: "room".to_string(),
                    name: "3F - Cupboard".to_string(),
                    kind: Missing::AddressedNothing,
                },
                Unresolved {
                    level: "zone".to_string(),
                    name: "Outdoors".to_string(),
                    kind: Missing::AddressedNothing,
                },
            ],
            "a typo and an empty room are DIFFERENT sentences, because an operator \
             sent looking for a room sitting in front of them acts on a lie"
        );
        assert!(routing.lamps.is_empty());
    }

    #[test]
    fn a_case_folded_name_is_a_typo_rather_than_a_name_to_forgive() {
        // WHICH IS HOW THE BRIDGE READS IT TOO. Forgiving case here would make
        // the routing depend on a rule the bridge's own listing does not follow.
        let routing = resolve(
            &stock(),
            &lights("[lights.room.\"3f - studio\"]\nshows = [\"done\"]\n"),
        );
        assert_eq!(routing.unresolved.len(), 1);
        assert_eq!(routing.unresolved[0].kind, Missing::NotOnBridge);
        assert!(routing.lamps.is_empty());
    }

    #[test]
    fn a_lamp_moved_to_another_room_answers_the_room_it_is_in_now() {
        // THE BRIDGE'S CURRENT MEMBERSHIP IS THE TRUTH AT RESOLVE TIME, which is
        // the case the whole join exists for: a lamp physically moved is not a
        // config to edit.
        let moved = CLIP_ROOMS.replace(
            r#"{"rid":"c97b44a9-cdcc-48c3-a15d-630fdaa936d0","rtype":"device"},"#,
            "",
        );
        assert_ne!(moved, CLIP_ROOMS, "the studio really lost HCL3's device");
        let held = inventory(&moved, CLIP_LIGHTS, CLIP_ZONES);
        let routing = resolve(
            &held,
            &lights("[lights.room.\"3F - Studio\"]\nshows = [\"done\"]\n"),
        );
        assert_eq!(
            carried(&routing, "3F - Studio - HCL3"),
            None,
            "the lamp left the room, so the room's declaration no longer reaches it"
        );
        assert_eq!(
            carried(&routing, "3F - Studio - HCL1"),
            Some(vec![Behaviour::Done])
        );
    }

    #[test]
    fn every_listing_is_fetched_and_a_bridge_that_refused_one_resolves_nothing() {
        // A LISTING THAT FAILED AND A LISTING THAT WAS EMPTY ARE DIFFERENT
        // ANSWERS. Collapsing them would resolve a config against an empty
        // inventory: every name reported as a typo, every lamp dark, all of it
        // stated confidently about a bridge that said nothing.
        let full = ScriptedBridge {
            rooms: Some(CLIP_ROOMS),
            lights: Some(CLIP_LIGHTS),
            zones: Some(CLIP_ZONES),
            gets: RefCell::new(Vec::new()),
            puts: RefCell::new(Vec::new()),
        };
        let map = resolve_on_bridge(
            &full,
            &lights("[lights.room.\"3F - Studio\"]\nshows = [\"done\"]\n"),
        );
        assert_eq!(map.map(|routing| routing.lamps.len()), Some(3));
        assert_eq!(
            full.gets.borrow().as_slice(),
            &["room".to_string(), "light".to_string(), "zone".to_string()],
        );
        let no_zones = ScriptedBridge {
            rooms: Some(CLIP_ROOMS),
            lights: Some(CLIP_LIGHTS),
            zones: None,
            gets: RefCell::new(Vec::new()),
            puts: RefCell::new(Vec::new()),
        };
        assert!(
            resolve_on_bridge(
                &no_zones,
                &lights("[lights.room.\"3F - Studio\"]\nshows = [\"done\"]\n")
            )
            .is_none(),
            "one refused listing resolves NOTHING rather than everything else"
        );
    }

    // --- the dim window ------------------------------------------------------

    /// 22:00 to 07:00, which is the window every room in the operator's own
    /// config carries.
    fn night(behaviours: &[Behaviour]) -> DimWindow {
        DimWindow {
            window: parse_window("22:00-07:00").expect("a window the parser takes"),
            behaviours: behaviours.to_vec(),
        }
    }

    const MIDNIGHT: Option<u16> = Some(0);
    const NOON: Option<u16> = Some(12 * 60);

    #[test]
    fn inside_a_window_an_enabled_behaviour_runs_dim_and_one_that_is_not_is_suppressed() {
        let window = night(&[Behaviour::Blocked, Behaviour::Unread, Behaviour::Looping]);
        assert_eq!(
            dim_showing(Some(&window), Behaviour::Blocked, MIDNIGHT),
            Showing::Dimmed,
            "an enabled behaviour runs its dim form"
        );
        assert_eq!(
            dim_showing(Some(&window), Behaviour::Done, MIDNIGHT),
            Showing::Dark,
            "and one the operator did not enable is taken away entirely: no strobes \
             while they are asleep"
        );
        assert_eq!(
            dim_showing(Some(&window), Behaviour::Done, NOON),
            Showing::Full,
            "outside the window everything runs full"
        );
        assert_eq!(
            dim_showing(None, Behaviour::Done, MIDNIGHT),
            Showing::Full,
            "and a lamp with no window at all is untouched at every hour, which is \
             what makes the whole feature opt-in"
        );
    }

    #[test]
    fn a_window_with_nothing_enabled_suppresses_every_behaviour_and_needs_no_mode() {
        // THE BEDROOM RULE, and it needs no special case in the code: the
        // operator's "never any light behaviour in here during quiet hours" is a
        // window with an empty enable list, which is already what an empty list
        // means everywhere else.
        let window = night(&[]);
        for behaviour in [
            Behaviour::Done,
            Behaviour::Failed,
            Behaviour::Blocked,
            Behaviour::Unread,
            Behaviour::Looping,
        ] {
            assert_eq!(
                dim_showing(Some(&window), behaviour, MIDNIGHT),
                Showing::Dark,
                "{behaviour:?} is suppressed by a window that enables nothing"
            );
            assert_eq!(
                dim_showing(Some(&window), behaviour, NOON),
                Showing::Full,
                "{behaviour:?} is untouched outside it"
            );
        }
    }

    #[test]
    fn a_clock_this_machine_cannot_read_is_treated_as_inside_the_window() {
        // FAIL CLOSED, which is `quiet_now`'s own direction: a flash at 3am is
        // what the window was set to prevent, and a missed signal costs nothing.
        assert_eq!(
            dim_showing(Some(&night(&[])), Behaviour::Done, None),
            Showing::Dark
        );
    }

    #[test]
    fn a_dim_window_nobody_can_parse_leaves_that_lamp_dark_and_says_which_lamp() {
        // FAIL CLOSED FOR THAT LAMP ALONE. An operator who asked for a dim
        // window and mistyped it would otherwise be flashed at 3am and told
        // nothing; the cost of the refusal is one lamp rather than the house.
        let routing = resolve(
            &stock(),
            &lights(
                "[lights.room.\"3F - Studio\"]\nshows = [\"done\"]\n\
                 dim_window = \"2200-0700\"\n\
                 [lights.room.\"2F - Kitchen\"]\nshows = [\"done\"]\n",
            ),
        );
        assert_eq!(
            routing.refusals,
            vec![
                "lights: `3F - Studio - HCL1` has dim_window \"2200-0700\", which is not \
                 a HH:MM-HH:MM window; that lamp stays dark"
                    .to_string(),
                "lights: `3F - Studio - HCL3` has dim_window \"2200-0700\", which is not \
                 a HH:MM-HH:MM window; that lamp stays dark"
                    .to_string(),
                "lights: `3F - Studio - HCL2` has dim_window \"2200-0700\", which is not \
                 a HH:MM-HH:MM window; that lamp stays dark"
                    .to_string(),
            ],
        );
        assert_eq!(
            carried(&routing, "3F - Studio - HCL1"),
            None,
            "the lamp is dark rather than signalling at an hour nobody could judge"
        );
        assert_eq!(
            carried(&routing, "2F - Kitchen - HCD3"),
            Some(vec![Behaviour::Done]),
            "and a lamp inheriting a readable declaration keeps its behaviours"
        );
    }

    // --- the mute ------------------------------------------------------------

    #[test]
    fn a_mute_reaches_a_lamp_by_its_own_name_by_its_room_and_by_any_zone_holding_it() {
        let hcl1 = stock()
            .lamps
            .into_iter()
            .find(|lamp| lamp.name == "3F - Studio - HCL1")
            .expect("HCL1 is in the listing");
        for typed in ["3F - Studio - HCL1", "3F - Studio", "Upstairs", "Desk"] {
            assert!(
                muted_now(&hcl1, &[typed.to_string()]),
                "{typed:?} must reach this lamp"
            );
        }
        assert!(
            !muted_now(&hcl1, &["2F - Kitchen".to_string()]),
            "and a name it does not answer to reaches nothing"
        );
        assert!(!muted_now(&hcl1, &[]), "an empty mute list mutes nothing");
    }

    #[test]
    fn the_names_a_mute_takes_are_every_name_a_declaration_writes_at_any_level() {
        assert_eq!(
            declared_names(&lights(
                "[lights.lamp.\"3F - Studio - HCL3\"]\nshows = [\"blocked\"]\n\
                 [lights.room.\"3F - Studio\"]\nshows = [\"done\"]\n\
                 [lights.zone.Upstairs]\nshows = [\"done\"]\n",
            )),
            vec![
                "3F - Studio".to_string(),
                "3F - Studio - HCL3".to_string(),
                "Upstairs".to_string(),
            ],
            "sorted, deduplicated, and with no level of its own: a mute names a place, \
             and every level is one"
        );
        assert!(declared_names(&lights("[lights]\n")).is_empty());
    }

    // --- the bodies ----------------------------------------------------------

    #[test]
    fn the_pulse_body_carries_the_locked_colour_duration_and_brightness() {
        // THE DECISION CARRIES THE VALUE, at the seam: this asserts what the
        // render layer WRITES for a done pulse, not that a constant equals
        // itself. Change any locked figure and this line changes with it.
        let shipped = lights("[lights]\n");
        let (color, pulse, brightness) =
            pulse_render(Behaviour::Done, &shipped, Showing::Full).expect("done is a pulse");
        assert_eq!(
            pulse_body(&pulse, color, brightness),
            r#"{"dimming":{"brightness":100.0},"signaling":{"colors":[{"xy":{"x":0.17,"y":0.7}}],"duration":4000,"signal":"on_off_color"}}"#,
            "deep green, four seconds, full brightness"
        );
        let (color, pulse, brightness) =
            pulse_render(Behaviour::Failed, &shipped, Showing::Full).expect("failed is a pulse");
        assert_eq!(
            pulse_body(&pulse, color, brightness),
            r#"{"dimming":{"brightness":100.0},"signaling":{"colors":[{"xy":{"x":0.675,"y":0.322}}],"duration":4000,"signal":"on_off_color"}}"#,
            "red, four seconds, full brightness"
        );
    }

    #[test]
    fn a_dimmed_pulse_fires_at_the_dim_floor_and_a_suppressed_one_does_not_fire() {
        let shipped = lights("[lights]\n");
        let (_, _, brightness) =
            pulse_render(Behaviour::Done, &shipped, Showing::Dimmed).expect("dimmed still fires");
        assert_eq!(
            brightness, shipped.dim.low,
            "the same blink at the faintest level the hardware has; a blink has no low \
             end to fade to, so the floor is the whole of what dim means for it"
        );
        assert!(
            pulse_render(Behaviour::Done, &shipped, Showing::Dark).is_none(),
            "and a suppressed pulse writes nothing at all"
        );
        for held in [Behaviour::Blocked, Behaviour::Unread, Behaviour::Looping] {
            assert!(
                pulse_render(held, &shipped, Showing::Full).is_none(),
                "{held:?} is a held state and has no pulse shape to fall back to"
            );
        }
    }

    #[test]
    fn each_held_state_renders_its_own_locked_colour_and_shape() {
        let shipped = lights("[lights]\n");
        let expected = [
            (
                crate::lights::Held::Blocked,
                crate::pulse::BLOCKED_COLOR,
                shipped.blocked,
            ),
            (
                crate::lights::Held::Looping,
                crate::pulse::LOOP_COLOR,
                shipped.looping.breath,
            ),
            (
                crate::lights::Held::UnreadFailure,
                crate::pulse::FAILURE_COLOR,
                shipped.unread.breath,
            ),
            (
                crate::lights::Held::UnreadSuccess,
                crate::pulse::UNREAD_SUCCESS_COLOR,
                shipped.unread.breath,
            ),
        ];
        for (held, color, breath) in expected {
            assert_eq!(
                held_render(held, &shipped, Showing::Full),
                (color, breath),
                "{held:?} runs its own colour at its own shape"
            );
            // THE DIM FORM IS ONE SHAPE FOR EVERY BEHAVIOUR, which is what the
            // operator locked: the colour still says which state it is, and only
            // the shape says the house is asleep.
            assert_eq!(
                held_render(held, &shipped, Showing::Dimmed),
                (color, shipped.dim),
                "{held:?} keeps its colour in the dim form"
            );
        }
        // THE LOCKED FIGURES, carried by the decision rather than echoed from a
        // constant: deep blue at 100 down to 30 in two-second fades, and the
        // violet and the two unread colours at 60 down to 10 in four-second ones.
        assert_eq!(
            held_render(crate::lights::Held::Blocked, &shipped, Showing::Full),
            (
                crate::pulse::PulseColor {
                    x: 0.1532,
                    y: 0.0475
                },
                crate::config::Breath {
                    duration_ms: 2000,
                    high: 100,
                    low: 30
                }
            )
        );
        assert_eq!(
            held_render(crate::lights::Held::Looping, &shipped, Showing::Full),
            (
                crate::pulse::PulseColor {
                    x: 0.213,
                    y: 0.0766
                },
                crate::config::Breath {
                    duration_ms: 4000,
                    high: 60,
                    low: 10
                }
            )
        );
        assert_eq!(
            held_render(crate::lights::Held::UnreadSuccess, &shipped, Showing::Full).0,
            crate::pulse::PulseColor { x: 0.50, y: 0.40 },
            "daylight for news that merely went unseen"
        );
        assert_eq!(
            held_render(crate::lights::Held::UnreadFailure, &shipped, Showing::Full).0,
            crate::pulse::PulseColor { x: 0.675, y: 0.322 },
            "and the failure pulse's own red for news that a run died"
        );
        assert_eq!(
            held_render(crate::lights::Held::Blocked, &shipped, Showing::Dimmed).1,
            crate::config::Breath {
                duration_ms: 3000,
                high: 7,
                low: 1
            },
            "the locked dim form"
        );
    }

    #[test]
    fn the_arm_states_the_colour_and_the_first_fade_and_every_fade_after_it_states_neither() {
        // ONE WRITE RATHER THAN TWO, because a colour write followed by a fade
        // is a visible jump: the lamp would land at whatever brightness it was
        // already at, in the new colour, before starting to move.
        let breath = crate::config::Breath {
            duration_ms: 2000,
            high: 100,
            low: 30,
        };
        let fades = crate::lights::breath_fades(12, &breath);
        assert_eq!(
            breath_arm_body(crate::pulse::BLOCKED_COLOR, &fades[0], breath.duration_ms),
            r#"{"color":{"xy":{"x":0.1532,"y":0.0475}},"dimming":{"brightness":30.0},"dynamics":{"duration":2000},"on":{"on":true}}"#,
        );
        assert_eq!(
            fade_body(&fades[1], breath.duration_ms),
            r#"{"dimming":{"brightness":100.0},"dynamics":{"duration":2000}}"#,
            "no colour and no `on`: the arm stated both, and repeating them is two \
             more fields the bridge reconciles mid-transition on every fade"
        );
    }

    #[test]
    fn what_puts_a_held_lamp_out_is_off_and_not_a_restore() {
        // Nothing snapshotted what the lamp was doing before the breath took it,
        // so there is nothing honest to put back.
        assert_eq!(clear_body(), r#"{"on":{"on":false}}"#);
        let bridge = bridge();
        clear_held(&bridge, &["light/a".to_string(), "light/b".to_string()]);
        assert_eq!(
            bridge.puts.borrow().as_slice(),
            &[
                ("light/a".to_string(), r#"{"on":{"on":false}}"#.to_string()),
                ("light/b".to_string(), r#"{"on":{"on":false}}"#.to_string()),
            ],
            "one PUT per held path, off the recorded names with no listing resolved"
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
}
