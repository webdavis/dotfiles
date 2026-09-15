//! Which channel one event is posted to, decided by lookup alone.
//!
//! A PURE LOOKUP OVER ONE MAP, because the subject picks the channel and
//! nothing else does: the destination reads the answer and posts, and the
//! order below is the whole policy rather than a branch per producer.
//!
//! SEVERITY OUTRANKS SUBJECT (operator ruling, 2026-09-14), which is why the
//! ROUTE is tried first: a producer that named `priority`, and a health event
//! that `routes::Kind::Health` routed there, both arrive carrying that route,
//! so one entry in this map sends every critical page to one channel whatever
//! project it was about. The two mechanisms never argue: the kind decides the
//! route before delivery, and this map only ever reads the route it was handed.

use std::collections::BTreeMap;

/// The channel map itself: an entry name or channel id per key, exactly as
/// the operator wrote it in `[plugins.discord.channels]`.
///
/// A PLAIN MAP RATHER THAN A NEWTYPE. Its keys are the operator's project
/// names, its values are opaque to this crate, and every invariant worth
/// holding (a required `default`, no empty value) is held where the config is
/// read, which is the layer that can name the key an operator has to fix.
pub type ChannelMap = BTreeMap<String, String>;

/// The catch-all key, and a REQUIRED one: a map without it silently swallows
/// the first event from every project nobody mapped.
pub const DEFAULT_KEY: &str = "default";

/// The channel this event posts to, or `None` for a map that states none.
///
/// FIRST HIT WINS over the keys below, in order. A step whose key is not in
/// the map falls through rather than failing, so a route nobody mapped still
/// reaches the project's own channel instead of nowhere.
///
/// THE DEFAULT ROUTE'S NAME IS AN ARGUMENT because it is the operator's to
/// choose (`[routes] default`): it is both the key an event with NO PROJECT
/// AT ALL lands on and the one route this lookup does not try ahead of the
/// project, and a copy compiled in here would disagree with their table.
pub fn channel_for<'a>(
    channels: &'a ChannelMap,
    route: &str,
    project: &str,
    default_route: &str,
) -> Option<&'a str> {
    keys_tried(route, project, default_route)
        .into_iter()
        .find_map(|key| channels.get(key).map(String::as_str))
}

/// Every key the lookup consults, in order, for this route and project.
///
/// THE FULL NAME AND THE BARE NAME ARE ONE RULE WITH TWO SPELLINGS: GitHub
/// only ever knows `webdavis/dotfiles` and pns's own events only ever know
/// `dotfiles`, so a project carrying a slash tries the whole thing first and
/// its basename second. Until somebody writes an `owner/name` key, both
/// spellings land on the same bare entry; the day `git subtree split` puts a
/// `pns` repository beside a `pns` directory, one config line separates them.
fn keys_tried<'a>(route: &'a str, project: &'a str, default_route: &'a str) -> Vec<&'a str> {
    let mut keys = Vec::with_capacity(4);
    if !route.is_empty() && route != default_route {
        keys.push(route);
    }
    if project.is_empty() {
        // DELIBERATELY NOT `DEFAULT_KEY`. "I know the project and have no
        // channel for it" and "there is no project" are two different
        // failures, and sending both to one channel loses the distinction
        // exactly when it is being looked for.
        keys.push(default_route);
    } else {
        keys.push(project);
        match project.rsplit_once('/') {
            Some((_, bare)) if !bare.is_empty() => keys.push(bare),
            _ => {}
        }
    }
    keys.push(DEFAULT_KEY);
    keys
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The route names this deployment happens to use. TEST-LOCAL, because
    /// the point of the change these pin is that no route name is this
    /// crate's to know.
    const DEFAULT_ROUTE: &str = "logbook";
    const URGENT_ROUTE: &str = "sirens";

    fn map(entries: &[(&str, &str)]) -> ChannelMap {
        entries
            .iter()
            .map(|(key, channel)| ((*key).to_string(), (*channel).to_string()))
            .collect()
    }

    fn mapped() -> ChannelMap {
        map(&[
            (DEFAULT_KEY, "catch-all"),
            (DEFAULT_ROUTE, "engine"),
            (URGENT_ROUTE, "pages"),
            ("dotfiles", "dotfiles-dev"),
        ])
    }

    /// `channel_for` with this deployment's own default route name.
    fn looked_up<'a>(channels: &'a ChannelMap, route: &str, project: &str) -> Option<&'a str> {
        channel_for(channels, route, project, DEFAULT_ROUTE)
    }

    #[test]
    fn a_named_route_beats_the_project_and_an_unnamed_one_does_not() {
        // THE MUTANT THIS PINS: the route dropped from the lookup, which sends
        // a critical page to the project's routine channel.
        assert_eq!(
            looked_up(&mapped(), URGENT_ROUTE, "dotfiles"),
            Some("pages")
        );
        assert_eq!(looked_up(&mapped(), "", "dotfiles"), Some("dotfiles-dev"));
        assert_eq!(
            looked_up(&mapped(), DEFAULT_ROUTE, "dotfiles"),
            Some("dotfiles-dev"),
            "the default route names no channel of its own; the project does"
        );
    }

    #[test]
    fn the_route_a_health_event_takes_is_the_key_that_wins() {
        // SEVERITY AHEAD OF SUBJECT, read off the routing rule rather than
        // restated here: whatever `Kind::Health` routes to is the key this
        // lookup consults first, so the two cannot disagree.
        let routes = crate::routes::Routes::named(DEFAULT_ROUTE, URGENT_ROUTE);
        let route = crate::routes::Kind::Health
            .route(&routes)
            .expect("health takes a route of its own");
        let channels = map(&[(DEFAULT_KEY, "catch-all"), (route, "pages")]);
        assert_eq!(looked_up(&channels, route, "dotfiles"), Some("pages"));
    }

    #[test]
    fn a_full_owner_name_key_beats_the_bare_name() {
        let channels = map(&[
            (DEFAULT_KEY, "catch-all"),
            ("webdavis/pns", "the-repository"),
            ("pns", "the-directory"),
        ]);
        assert_eq!(
            looked_up(&channels, "", "webdavis/pns"),
            Some("the-repository")
        );
        assert_eq!(
            looked_up(&channels, "", "pns"),
            Some("the-directory"),
            "a bare project name never reads an `owner/name` entry"
        );
    }

    #[test]
    fn a_full_name_nobody_mapped_falls_back_to_its_bare_entry() {
        assert_eq!(
            looked_up(&mapped(), "", "webdavis/dotfiles"),
            Some("dotfiles-dev")
        );
    }

    #[test]
    fn an_unmapped_project_reaches_the_catch_all_and_no_project_reaches_the_engine() {
        // THE TWO FALLBACKS ARE DIFFERENT CHANNELS ON PURPOSE: a project
        // nobody mapped and no project at all are two failures, and folding
        // them together loses which one happened.
        assert_eq!(looked_up(&mapped(), "", "netpulse"), Some("catch-all"));
        assert_eq!(looked_up(&mapped(), "", ""), Some("engine"));
        assert_eq!(
            looked_up(&map(&[(DEFAULT_KEY, "catch-all")]), "", ""),
            Some("catch-all"),
            "and with no engine entry written, the catch-all still answers"
        );
    }

    #[test]
    fn the_no_project_key_is_whatever_the_config_calls_the_default_route() {
        // THE MUTANT THIS PINS: the argument ignored for a compiled-in name.
        // A deployment that renamed its default route would otherwise lose
        // every event with no project to the catch-all.
        let channels = map(&[(DEFAULT_KEY, "catch-all"), ("pns-events", "stale-name")]);
        assert_eq!(
            channel_for(&channels, "", "", DEFAULT_ROUTE),
            Some("catch-all")
        );
        assert_eq!(
            channel_for(&channels, "", "", "pns-events"),
            Some("stale-name")
        );
    }

    #[test]
    fn a_map_stating_nothing_answers_nothing() {
        assert_eq!(
            looked_up(&ChannelMap::new(), URGENT_ROUTE, "dotfiles"),
            None
        );
    }
}
