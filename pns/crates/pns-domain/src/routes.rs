//! What pns's routes are called, which is the operator's to say and not this
//! crate's.
//!
//! NO COMPILED ROSTER, and that is the whole point (operator ruling,
//! 2026-09-15). pns and every producer that posts through it are separate
//! tools that learn about each other when somebody configures them together
//! and never before, so a route name compiled in here would be one
//! deployment's gateway baked into a product other people install. Which
//! routes exist is the set of signing keys `[plugins.hermes.keys]` grants: a
//! route the operator granted a key to is a route they granted, and a route
//! with no key is refused rather than signed with somebody else's. The two
//! routes pns SELECTS for itself are named in `[routes]`.
//!
//! WHAT IS STILL pns'S OWN, because it names no other tool: the producer
//! words `agent` and `health`, the rule that a health event takes the urgent
//! route rather than the routine one, and the refusal above.

/// What the two routes pns selects for itself are called.
///
/// TWO NAMES AND NOT A LIST, because these are the only routes pns picks
/// without being told: everything else arrives already named by the producer
/// that raised it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Routes {
    default: String,
    urgent: String,
}

impl Routes {
    /// `[routes]` as the operator wrote it.
    ///
    /// NO VALIDATION HERE. A name that could not stand as a URL path segment
    /// is refused BY NAME where the table is read, which is the layer that
    /// can tell the operator which key to fix; a second, quieter rule here
    /// would be a route silently swapped for another.
    pub fn named(default: &str, urgent: &str) -> Self {
        Routes {
            default: default.to_string(),
            urgent: urgent.to_string(),
        }
    }

    /// The route an event whose producer named none takes.
    ///
    /// IT IS ALSO THE FINAL SEGMENT OF THE DEFAULT GATEWAY URL, which
    /// `channel_url` swaps for whatever this says, so the two cannot disagree
    /// however the table is written.
    pub fn default_route(&self) -> &str {
        &self.default
    }

    /// The route reserved for what needs a human now.
    pub fn urgent_route(&self) -> &str {
        &self.urgent
    }
}

impl Default for Routes {
    /// The names this repository's own gateway uses, and the ONE place either
    /// string is written: they are defaults a config overrides, never a
    /// roster a config is checked against.
    fn default() -> Self {
        Routes::named("pns-events", "priority")
    }
}

/// What an event IS, which is what decides where it lands when its producer
/// named no route.
///
/// A PRODUCER NAMES A KIND, NEVER A ROUTE (operator ruling, 2026-09-15). A
/// producer knows its own work failed and nothing about the gateway's
/// channels, so the word it sends is what the failure is; this mapping is
/// what turns that into a route, and WHICH ROUTE A KIND TAKES is fixed even
/// though the route's NAME is not: `health` is defined as a machine's own
/// health, so a mapping that sent one to the routine route would contradict
/// the definition.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Kind {
    /// A session event: a harness hook, the shell notifier, a daemon job.
    #[default]
    Agent,
    /// A machine's own health, such as an unattended upgrade that failed while
    /// nobody was watching. It pages.
    Health,
}

impl Kind {
    /// The kind a producer spelled, or `None` for a word that is neither.
    /// REFUSED RATHER THAN DEFAULTED: a typo that quietly became `agent` would
    /// route a page nobody reads in time to the routine channel.
    pub fn from_word(word: &str) -> Option<Self> {
        match word {
            "agent" => Some(Self::Agent),
            "health" => Some(Self::Health),
            _ => None,
        }
    }

    /// The route this kind takes when the event named none, or `None` when it
    /// takes the default route, which is the empty route every path already
    /// reads as the default.
    ///
    /// THE STATE IS THE SECOND AXIS (operator ruling, 2026-09-14: the subject
    /// picks the channel and severity overrides it). A health event pages only
    /// when somebody is waiting on it: an upgrade that failed while nobody
    /// watched is why the urgent route exists, and one that went fine is a
    /// line in the weekly record. The list is `missed::NEEDS_YOU`, the one
    /// place this crate says which states wait on the operator, so a page and
    /// the recap's own NEEDS YOU section cannot disagree about what urgent is.
    pub fn route<'a>(self, routes: &'a Routes, state: &str) -> Option<&'a str> {
        match self {
            Self::Agent => None,
            Self::Health if crate::missed::NEEDS_YOU.contains(&state) => {
                Some(routes.urgent_route())
            }
            Self::Health => None,
        }
    }

    /// Every word `from_word` accepts, for a usage line and for the tests.
    pub const WORDS: &'static [&'static str] = &["agent", "health"];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_health_event_takes_the_urgent_route_whatever_the_config_calls_it() {
        let named = Routes::named("logbook", "sirens");
        assert_eq!(Kind::Health.route(&named, "failed"), Some("sirens"));
        assert_eq!(
            Kind::Agent.route(&named, "failed"),
            None,
            "the default route is the empty route, not a second spelling of it"
        );
        assert_eq!(Kind::default(), Kind::Agent);
    }

    #[test]
    fn a_health_event_nobody_has_to_answer_stays_off_the_urgent_route() {
        // THE MUTANT THIS PINS: the state ignored, which would page the
        // operator for a weekly upgrade that went fine.
        let named = Routes::named("logbook", "sirens");
        for state in ["done", "resolved", "observation", "progress", ""] {
            assert_eq!(
                Kind::Health.route(&named, state),
                None,
                "`{state}` is nobody waiting on the operator"
            );
        }
        for state in crate::missed::NEEDS_YOU {
            assert_eq!(
                Kind::Health.route(&named, state),
                Some("sirens"),
                "`{state}` is the operator being waited on"
            );
        }
    }

    #[test]
    fn the_two_route_names_are_read_from_configuration() {
        let named = Routes::named("logbook", "sirens");
        assert_eq!(named.default_route(), "logbook");
        assert_eq!(named.urgent_route(), "sirens");
    }

    #[test]
    fn the_shipped_names_are_defaults_and_nothing_else_states_them() {
        // THE DEFAULTS ARE THIS REPOSITORY'S OWN GATEWAY, which is exactly
        // what a default is for; a config naming other routes must not have to
        // agree with them.
        let shipped = Routes::default();
        assert_eq!(shipped.default_route(), "pns-events");
        assert_eq!(shipped.urgent_route(), "priority");
        assert_ne!(shipped, Routes::named("logbook", "sirens"));
    }

    #[test]
    fn only_the_two_words_name_a_kind() {
        for word in Kind::WORDS {
            assert!(Kind::from_word(word).is_some(), "`{word}` names no kind");
        }
        for word in ["", "agents", "Health", "priority", "--kind"] {
            assert_eq!(Kind::from_word(word), None, "`{word}` is not a kind");
        }
    }

    #[test]
    fn every_default_route_name_can_stand_as_a_url_path_segment() {
        // A name that could not become a path segment would be a default no
        // post could ever use.
        let shipped = Routes::default();
        for route in [shipped.default_route(), shipped.urgent_route()] {
            assert!(
                crate::safety::route_name_is_usable(route),
                "`{route}` cannot stand as a route"
            );
        }
    }
}
