//! The event a producer raised, as the value every destination is handed.
//!
//! THE WIRE FORMAT IS NOT HERE. How this becomes the channel contract's JSON
//! object is the executable destination's business and lives with it: the two
//! change for different reasons, the FORMAT when the contract does and this
//! struct when a producer starts carrying a new field.

/// One rendered event, the structured form of the channel contract's JSON
/// object. The pane is the SANITIZED one.
#[derive(Debug, Default, PartialEq)]
pub struct Event {
    pub agent: String,
    pub state: String,
    pub project: String,
    pub branch: String,
    pub detail: String,
    pub title: String,
    /// The harness session this event belongs to, for a channel that says
    /// WHICH session sent it. Empty on every path with no session.
    pub session: String,
    /// What that session was asked to do, read back from the sessions store.
    pub session_title: String,
    pub message: String,
    pub preview: String,
    pub pane: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DeliveryScope {
    #[default]
    Automatic,
    LocalOnly,
    RemoteOnly,
}

/// The parsed event arguments. Scope defaults to automatic; other fields default to empty or false, so a
/// bare invocation is valid and renders an empty event.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EventArgs {
    pub agent: String,
    pub state: String,
    pub project: String,
    pub branch: String,
    pub detail: String,
    pub pane: String,
    /// The named hermes route this event posts to, resolved through the
    /// config's `[plugins.hermes]` channels table; empty means the default
    /// (alert) route. Names, not URLs: the caller says WHERE, the config
    /// says HOW to get there.
    pub channel: String,
    /// The harness session this event belongs to, as the payload named it.
    /// Empty on every path with no session: argv, a producer submission, a
    /// job the daemon woke.
    pub session: String,
    /// What that session was asked to do, already stored and read back.
    pub session_title: String,
    pub scope: crate::DeliveryScope,
    /// The >=300s tier: the lights signal rides on top of whatever else the
    /// plan decides.
    pub long_running: bool,
    /// What this event IS, which is what picks its route when `channel` is
    /// empty. Producers name a kind; nobody outside this crate names a route
    /// it has not been told.
    pub kind: crate::routes::Kind,
}

impl EventArgs {
    /// This event with its route settled: the one its producer named, else the
    /// one its kind names, else the default.
    ///
    /// THE NAMED ROUTE WINS. A producer that said where already answered the
    /// question the kind is here to answer.
    ///
    /// RESOLVED ONCE, AND EARLY. `channel` is what the ledger row, the retry
    /// that rebuilds off it and every destination read, so a route filled in
    /// later would leave a page recorded on one route and posted to another.
    pub fn routed(mut self, routes: &crate::routes::Routes) -> Self {
        if self.channel.is_empty()
            && let Some(route) = self.kind.route(routes)
        {
            self.channel = route.to_string();
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routes::{Kind, Routes};

    fn health() -> EventArgs {
        EventArgs {
            kind: Kind::Health,
            ..EventArgs::default()
        }
    }

    #[test]
    fn a_health_event_is_routed_to_the_urgent_route_the_config_named() {
        let routed = health().routed(&Routes::named("logbook", "sirens"));
        assert_eq!(routed.channel, "sirens");
    }

    #[test]
    fn a_route_the_producer_named_survives_its_kind() {
        let named = EventArgs {
            channel: "posture-pages".to_string(),
            ..health()
        };
        assert_eq!(
            named.routed(&Routes::named("logbook", "sirens")).channel,
            "posture-pages",
            "the kind overrode a route the producer had already named"
        );
    }

    #[test]
    fn a_session_event_is_left_on_the_empty_route_every_path_reads_as_default() {
        let routed = EventArgs::default().routed(&Routes::named("logbook", "sirens"));
        assert!(
            routed.channel.is_empty(),
            "an agent event was pinned to a route name: {}",
            routed.channel
        );
    }
}
