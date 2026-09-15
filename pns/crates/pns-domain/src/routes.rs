//! The hermes routes pns posts to, named once.
//!
//! ONE ROSTER, because three layers ask the same question and used to answer
//! it with three literals: the config schema decides which route names
//! `[plugins.hermes.keys]` may carry, the signer looks a key up by the route
//! it is about to post to, and the failure wording quotes that key back at the
//! operator. A route added in one of the three and missed in the others is a
//! key the config refuses, or one nothing ever reads.

/// The route an event that named none takes: the gateway's own `pns-events`
/// webhook, which is also the final segment of `DEFAULT_HERMES_URL`. THE
/// RETURN RECAP TAKES IT TOO. The `pns-recap` route and its Discord channel
/// retired on 2026-09-15, and a recap is a session event like any other.
pub const DEFAULT_ROUTE: &str = "pns-events";

/// The route a page submitted by the posture pipeline posts to. NOTHING IN
/// PNS SELECTS IT: posture is the producer that names it, and this is the wire
/// name its own `severity_route` spells
/// (`posture/crates/posture-domain/src/severity.rs`). It is here because the
/// roster is what grants a route a key, and a route pns has no key for is a
/// posture page pns refuses to sign.
pub const POSTURE_ROUTE: &str = "posture-pages";

/// Every route pns posts to, each verified by its OWN signing key: one
/// compromised key reaches one Discord channel rather than all of them.
///
/// A ROUTE NOT IN THIS LIST HAS NO KEY, and a post to one is refused rather
/// than signed with somebody else's: `--channel` takes any usable name, so
/// falling back to a shared key would sign for a route nobody granted.
pub const ROUTES: &[&str] = &[DEFAULT_ROUTE, POSTURE_ROUTE, crate::stale::PRIORITY_ROUTE];

/// What an event IS, which is what decides where it lands when its producer
/// named no route.
///
/// A PRODUCER NAMES A KIND, NEVER A ROUTE (operator ruling, 2026-09-15). uu
/// knows its lane failed and nothing about Discord channels, so the word it
/// sends is what the failure is; this mapping is what turns that into a route,
/// and it is FIXED rather than configurable for the same reason the stale
/// escalation's route is: `priority` is defined as machine health and security,
/// so a config that pointed a health event elsewhere would contradict the
/// definition.
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
    /// reads as `DEFAULT_ROUTE`.
    pub fn route(self) -> Option<&'static str> {
        match self {
            Self::Agent => None,
            Self::Health => Some(crate::stale::PRIORITY_ROUTE),
        }
    }

    /// Every word `from_word` accepts, for a usage line and for the tests.
    pub const WORDS: &'static [&'static str] = &["agent", "health"];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_health_event_pages_and_an_agent_event_takes_the_default() {
        assert_eq!(Kind::Health.route(), Some(crate::stale::PRIORITY_ROUTE));
        assert_eq!(
            Kind::Agent.route(),
            None,
            "the default route is the empty route, not a second spelling of it"
        );
        assert_eq!(Kind::default(), Kind::Agent);
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
    fn every_route_a_kind_takes_is_on_the_roster() {
        // A kind routing somewhere the roster does not grant a key would be a
        // post pns refuses at the last moment instead of a page.
        for kind in [Kind::Agent, Kind::Health] {
            let route = kind.route().unwrap_or(DEFAULT_ROUTE);
            assert!(ROUTES.contains(&route), "`{route}` has no signing key");
        }
    }

    #[test]
    fn every_route_name_can_stand_as_a_url_path_segment() {
        // The roster is what the schema admits, so a name that could not
        // become a path segment would be a key an operator may write and no
        // post could ever use.
        for route in ROUTES {
            assert!(
                crate::safety::route_name_is_usable(route),
                "`{route}` cannot stand as a route"
            );
        }
    }

    #[test]
    fn the_roster_names_each_route_once() {
        let mut sorted: Vec<&&str> = ROUTES.iter().collect();
        sorted.sort_unstable();
        let mut unique = sorted.clone();
        unique.dedup();
        assert_eq!(sorted, unique, "a duplicated route is a duplicated key");
    }
}
