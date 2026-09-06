//! What the bridge has, and what a config named that it does not.

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
