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

/// Where an event with NO PROJECT AT ALL lands, which is the engine's own
/// channel.
///
/// DELIBERATELY NOT `DEFAULT_KEY`. "I know the project and have no channel for
/// it" and "there is no project" are two different failures, and sending both
/// to one channel loses the distinction exactly when it is being looked for.
pub const NO_PROJECT_KEY: &str = crate::routes::DEFAULT_ROUTE;

/// The channel this event posts to, or `None` for a map that states none.
///
/// FIRST HIT WINS over the keys below, in order. A step whose key is not in
/// the map falls through rather than failing, so a route nobody mapped still
/// reaches the project's own channel instead of nowhere.
pub fn channel_for<'a>(channels: &'a ChannelMap, route: &str, project: &str) -> Option<&'a str> {
    keys_tried(route, project)
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
fn keys_tried<'a>(route: &'a str, project: &'a str) -> Vec<&'a str> {
    let mut keys = Vec::with_capacity(4);
    if !route.is_empty() && route != crate::routes::DEFAULT_ROUTE {
        keys.push(route);
    }
    if project.is_empty() {
        keys.push(NO_PROJECT_KEY);
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

    fn map(entries: &[(&str, &str)]) -> ChannelMap {
        entries
            .iter()
            .map(|(key, channel)| ((*key).to_string(), (*channel).to_string()))
            .collect()
    }

    fn mapped() -> ChannelMap {
        map(&[
            (DEFAULT_KEY, "catch-all"),
            (NO_PROJECT_KEY, "engine"),
            ("priority", "pages"),
            ("dotfiles", "dotfiles-dev"),
        ])
    }

    #[test]
    fn a_named_route_beats_the_project_and_an_unnamed_one_does_not() {
        // THE MUTANT THIS PINS: the route dropped from the lookup, which sends
        // a critical page to the project's routine channel.
        assert_eq!(
            channel_for(&mapped(), "priority", "dotfiles"),
            Some("pages")
        );
        assert_eq!(channel_for(&mapped(), "", "dotfiles"), Some("dotfiles-dev"));
        assert_eq!(
            channel_for(&mapped(), NO_PROJECT_KEY, "dotfiles"),
            Some("dotfiles-dev"),
            "the default route names no channel of its own; the project does"
        );
    }

    #[test]
    fn the_route_a_health_event_takes_is_the_key_that_wins() {
        // SEVERITY AHEAD OF SUBJECT, read off the routing rule rather than
        // restated here: whatever `Kind::Health` routes to is the key this
        // lookup consults first, so the two cannot disagree.
        let route = crate::routes::Kind::Health
            .route()
            .expect("health takes a route of its own");
        let channels = map(&[(DEFAULT_KEY, "catch-all"), (route, "pages")]);
        assert_eq!(channel_for(&channels, route, "dotfiles"), Some("pages"));
    }

    #[test]
    fn a_full_owner_name_key_beats_the_bare_name() {
        let channels = map(&[
            (DEFAULT_KEY, "catch-all"),
            ("webdavis/pns", "the-repository"),
            ("pns", "the-directory"),
        ]);
        assert_eq!(
            channel_for(&channels, "", "webdavis/pns"),
            Some("the-repository")
        );
        assert_eq!(
            channel_for(&channels, "", "pns"),
            Some("the-directory"),
            "a bare project name never reads an `owner/name` entry"
        );
    }

    #[test]
    fn a_full_name_nobody_mapped_falls_back_to_its_bare_entry() {
        assert_eq!(
            channel_for(&mapped(), "", "webdavis/dotfiles"),
            Some("dotfiles-dev")
        );
    }

    #[test]
    fn an_unmapped_project_reaches_the_catch_all_and_no_project_reaches_the_engine() {
        // THE TWO FALLBACKS ARE DIFFERENT CHANNELS ON PURPOSE: a project
        // nobody mapped and no project at all are two failures, and folding
        // them together loses which one happened.
        assert_eq!(channel_for(&mapped(), "", "netpulse"), Some("catch-all"));
        assert_eq!(channel_for(&mapped(), "", ""), Some("engine"));
        assert_eq!(
            channel_for(&map(&[(DEFAULT_KEY, "catch-all")]), "", ""),
            Some("catch-all"),
            "and with no engine entry written, the catch-all still answers"
        );
    }

    #[test]
    fn a_map_stating_nothing_answers_nothing() {
        assert_eq!(
            channel_for(&ChannelMap::new(), "priority", "dotfiles"),
            None
        );
    }
}
