//! Which fixture each behaviour routes to, and what it last showed.

use super::dim::DimWindow;
use super::inventory::Missing;
use super::inventory::{Inventory, Lamp, Unresolved};
use super::window::parse_window;
use super::window::window_refusal;

/// One lamp with every question answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Routed {
    pub lamp: Lamp,
    pub shows: Vec<crate::lamps::config::Behaviour>,
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
pub const LEVELS: [&str; 3] = ["lamp", "room", "zone"];
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
pub fn resolve(inventory: &Inventory, lights: &crate::lamps::config::Lights) -> Routing {
    let mut routing = Routing {
        unresolved: unresolved_names(inventory, lights),
        ..Routing::default()
    };
    for lamp in &inventory.lamps {
        let shows = match winner(&mut routing, lamp, lights, "shows", |target| {
            target.shows.clone()
        }) {
            // A CONTESTED BEHAVIOUR SET IS AN EMPTY ONE, which the drop below
            // turns into a dark lamp: two declarations that each name what it
            // carries settle nothing, so it carries nothing.
            Answered::Refused => Vec::new(),
            Answered::Silent => Vec::new(),
            Answered::Stated(shows) => shows,
        };
        let dim = match winner(&mut routing, lamp, lights, "dim_window", |target| {
            target
                .dim_window
                .as_ref()
                .map(|stated| (stated.clone(), target.dim_behaviours.clone()))
        }) {
            // A CONTESTED DIM QUESTION FAILS DARK, exactly as an unreadable one
            // does below, and telling the two apart from SILENCE is the whole
            // reason this answer has three arms. Collapsed into one `None` they
            // took the no-window path, which is FULL BRIGHTNESS: the config
            // that said loudest that a lamp must be quiet at night, two
            // declarations both stating when, was the one that ran it at full
            // brightness all night.
            Answered::Refused => continue,
            Answered::Silent => None,
            Answered::Stated((stated, behaviours)) => match parse_window(&stated) {
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
/// ONE REFUSAL PER PROBLEM, not per lamp that met it: two lamps inheriting one
/// room's unreadable window is one typo, and saying it twice trains an operator
/// to skim the line.
pub fn remember(refusals: &mut Vec<String>, refusal: String) {
    if !refusals.contains(&refusal) {
        refusals.push(refusal);
    }
}

/// The most specific declaration that STATES one question, or a refusal when
/// two zones both do.
fn winner<Answer>(
    routing: &mut Routing,
    lamp: &Lamp,
    lights: &crate::lamps::config::Lights,
    question: &str,
    stated: impl Fn(&crate::lamps::config::Target) -> Option<Answer>,
) -> Answered<Answer> {
    for level in LEVELS {
        let answers: Vec<(&String, Answer)> = declarations(lamp, lights, level)
            .filter_map(|(name, target)| Some((name, stated(target)?)))
            .collect();
        match answers.len() {
            0 => continue,
            1 => {
                return answers
                    .into_iter()
                    .next()
                    .map_or(Answered::Silent, |(_, answer)| Answered::Stated(answer));
            }
            _ => {
                let names: Vec<String> = answers
                    .iter()
                    .map(|(name, _)| format!("{name:?}"))
                    .collect();
                remember(
                    &mut routing.refusals,
                    double_cover_refusal(&lamp.name, level, question, &names),
                );
                return Answered::Refused;
            }
        }
    }
    Answered::Silent
}
/// Every declared name the bridge could not answer, and why.
fn unresolved_names(
    inventory: &Inventory,
    lights: &crate::lamps::config::Lights,
) -> Vec<Unresolved> {
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

/// What the declarations had to say about one question for one lamp.
///
/// THREE ANSWERS AND NOT AN `Option`, because "nobody stated this" and "two
/// declarations stated it and neither can win" are different facts with
/// different fail directions, and every caller has to choose between them. As
/// one `None` the refusal took the silent path, which on the dim question is
/// full brightness.
enum Answered<Answer> {
    Stated(Answer),
    Silent,
    Refused,
}
/// Every declaration at one level that names this lamp.
fn declarations<'settings>(
    lamp: &'settings Lamp,
    lights: &'settings crate::lamps::config::Lights,
    level: &str,
) -> impl Iterator<Item = (&'settings String, &'settings crate::lamps::config::Target)> {
    let (table, names): (
        &std::collections::BTreeMap<String, crate::lamps::config::Target>,
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
fn double_cover_refusal(lamp: &str, level: &str, question: &str, names: &[String]) -> String {
    format!(
        "lights: `{lamp}` is covered by {} {level} declarations that each state \
         `{question}` ({}); there is nothing more specific to break the tie, so \
         that lamp answers none of them",
        names.len(),
        names.join(" and ")
    )
}
