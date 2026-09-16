//! What a configuration file is REFUSED for, once the schema has admitted
//! every key in it.
//!
//! AT LOAD, WHICH BLOCKS THE WHOLE FILE, and that is the point of gathering
//! them here: each of these is a config that would otherwise load happily and
//! then misdeliver, one event at a time, in a way nobody notices. The file is
//! unusable until the named line is written, and the machine runs on its
//! built-in core meanwhile.

use super::*;

/// The plugins that are each a DURABLE LOG, READ OFF THE ROSTER rather than
/// restated here: `pns_domain::registry::ROSTER` is the single source of
/// truth for which channel declares `Routing { durable: true, .. }`, so a
/// third one added there is caught here too instead of loading silently
/// alongside the other two and posting every event twice.
fn durable_plugin_names() -> impl Iterator<Item = &'static str> {
    pns_domain::registry::ROSTER
        .iter()
        .filter_map(|registration| match registration.kind {
            pns_domain::registry::PluginKind::Channel(routing) if routing.durable => {
                Some(registration.name)
            }
            _ => None,
        })
}

/// Refuses a config that switches more than one durable log on, naming every
/// one enabled.
///
/// A MEASURED DUPLICATE, not caution. `channel_plan` keeps every plugin whose
/// declaration passes the selection, so two durable channels produce two legs
/// and one event reaches Discord twice; `Destinations::durable()` answers the
/// FIRST durable entry in registration order, and the recap posts through
/// exactly that call, so the recap would silently pick whichever registered
/// first while every other event doubled.
///
/// AT LOAD, WHICH BLOCKS THE WHOLE FILE, rather than in the registry: a
/// refusal there selects the entire roster and turns all of them on, which is
/// the opposite of what this refuses. Here the file is unusable, the sentence
/// names every enabled table, and the machine falls back to the core until
/// all but one of the lines is edited.
pub(super) fn refuse_two_durable_logs(config: &Config) -> Result<(), ConfigError> {
    let enabled: Vec<&str> = durable_plugin_names()
        .filter(|name| config.plugins.get(*name).is_some_and(|entry| entry.enabled))
        .collect();
    if enabled.len() <= 1 {
        return Ok(());
    }
    let named = enabled
        .iter()
        .map(|name| format!("`[plugins.{name}]`"))
        .collect::<Vec<_>>()
        .join(" and ");
    Err(ConfigError::Invalid(format!(
        "{named} are all enabled and only one durable log may be: \
         two of them post every event twice, and the recap goes to whichever registered first. \
         Switch all but one off."
    )))
}

/// Refuses an ARMED discord table whose map states no catch-all.
///
/// `default` IS REQUIRED because every lookup ends there: a map without one
/// silently swallows the first event from every project nobody mapped, which
/// is the one failure a paper trail cannot afford. It is refused AT LOAD, with
/// the two-durable-logs refusal beside it, so the file is unusable until the
/// line is written rather than each event failing one at a time.
///
/// A TABLE THAT IS OFF, OR NAMES NO BACKEND, IS NOT THIS REFUSAL'S BUSINESS:
/// the first is the operator's own switch and the second already has a
/// sentence of its own, which this one would only double.
pub(super) fn refuse_a_map_without_a_catch_all(config: &Config) -> Result<(), ConfigError> {
    let Ok(Some(settings)) = armed_discord(config) else {
        return Ok(());
    };
    if super::states_default_channel(settings) {
        return Ok(());
    }
    Err(ConfigError::Invalid(
        "`[plugins.discord.channels]` states no `default`: it is the catch-all every lookup \
         ends at, and a map without one swallows the first event from every project nobody \
         mapped. Write a `default` entry."
            .to_string(),
    ))
}

/// Refuses an ARMED discord table whose map states no channel for the urgent
/// route.
///
/// THE SEVERITY AXIS IS THE ONE THAT CANNOT DEGRADE QUIETLY (operator ruling,
/// 2026-09-14): the route a critical page carries is the first key
/// `pns_domain::channel_map::channel_for` consults, so a map that names no
/// channel under it does not fail, it falls through to the project's own
/// routine channel. A page nobody reads in time reads exactly like a page
/// nobody sent, which is why this is refused at load beside the catch-all
/// rather than left to show up one event at a time.
///
/// THE ROUTE'S NAME COMES FROM `[routes] urgent`, never from a copy here, so
/// the refusal names the key the operator actually has to write.
pub(super) fn refuse_a_map_without_the_urgent_channel(config: &Config) -> Result<(), ConfigError> {
    let Ok(Some(settings)) = armed_discord(config) else {
        return Ok(());
    };
    let urgent = config.routes.urgent_route();
    if super::states_channel(settings, urgent) {
        return Ok(());
    }
    Err(ConfigError::Invalid(format!(
        "`[plugins.discord.channels]` states no `{urgent}`: it is the channel every critical \
         page takes whatever project it was about, and a map without one sends those pages to \
         the project's own routine channel instead. Write a `{urgent}` entry."
    )))
}

#[cfg(test)]
mod urgent_channel_tests {
    use super::super::{ConfigError, parse_config};

    /// Every channel id in this module is an OBVIOUS FAKE. A real one is a
    /// secret and lives in KeePassXC by entry name; nothing here may hold one.
    const FAKE: &str = "000000000000000000";

    /// THE MUTANT THIS PINS: the requirement dropped, which loads a map where
    /// the severity axis silently degrades. A critical page about a mapped
    /// project would land in that project's routine channel, which is the one
    /// place a page cannot be allowed to go quiet.
    #[test]
    fn an_armed_map_with_no_urgent_channel_is_refused_naming_the_route() {
        for (text, route) in [
            (
                format!(
                    "[plugins.discord]\nenabled = true\ntype = \"bot\"\n\
                     [plugins.discord.channels]\ndefault = \"{FAKE}\"\ndotfiles = \"{FAKE}\"\n"
                ),
                "priority",
            ),
            (
                format!(
                    "[routes]\nurgent = \"sirens\"\n\
                     [plugins.discord]\nenabled = true\ntype = \"bot\"\n\
                     [plugins.discord.channels]\ndefault = \"{FAKE}\"\n"
                ),
                "sirens",
            ),
            (
                format!(
                    "[plugins.discord]\nenabled = true\ntype = \"bot\"\n\
                     [plugins.discord.channels]\ndefault = \"{FAKE}\"\npriority = \"\"\n"
                ),
                "priority",
            ),
        ] {
            let Err(ConfigError::Invalid(said)) = parse_config(&text) else {
                panic!("case: {text:?}");
            };
            // THE ROUTE'S OWN NAME, read off `[routes] urgent` rather than
            // compiled in: an operator who renamed the route has to be told
            // which key to write, not the name this repository happens to use.
            assert!(said.contains(route), "{said}");
            assert!(said.contains("[plugins.discord.channels]"), "{said}");
        }
    }

    #[test]
    fn a_map_naming_the_urgent_route_loads_and_a_switched_off_table_is_never_asked() {
        // The positive control: a refusal that fired on either of these would
        // take the whole config down over a channel the operator did arm, or
        // over one nobody posts to.
        for text in [
            format!(
                "[plugins.discord]\nenabled = true\ntype = \"bot\"\n\
                 [plugins.discord.channels]\ndefault = \"{FAKE}\"\npriority = \"{FAKE}\"\n"
            ),
            format!(
                "[routes]\nurgent = \"sirens\"\n\
                 [plugins.discord]\nenabled = true\ntype = \"bot\"\n\
                 [plugins.discord.channels]\ndefault = \"{FAKE}\"\nsirens = \"{FAKE}\"\n"
            ),
            "[plugins.discord]\nenabled = false\ntype = \"bot\"\n".to_string(),
        ] {
            assert!(parse_config(&text).is_ok(), "case: {text:?}");
        }
    }
}

#[cfg(test)]
mod durable_tests {
    use super::super::{ConfigError, parse_config};

    /// The two entries an armed map cannot load without: the catch-all every
    /// lookup ends at, and the channel the urgent route takes.
    const CATCH_ALL: &str = "[plugins.discord.channels]\ndefault = \"9001\"\npriority = \"9002\"\n";

    /// THE MUTANT THIS PINS: the refusal dropped, which is a config that loads
    /// happily and posts every event to Discord twice while the recap follows
    /// whichever destination registered first.
    #[test]
    fn a_config_enabling_both_durable_logs_is_refused_naming_both() {
        let refusal = parse_config(
            "[plugins.hermes]\nenabled = true\n[plugins.discord]\nenabled = true\ntype = \"bot\"\n",
        )
        .expect_err("two durable logs is refused");
        let ConfigError::Invalid(said) = refusal else {
            panic!("a schema refusal, not a parse failure");
        };
        // BOTH TABLES, because the fix is one line in either of them and an
        // operator reading only one name would go and edit the one that is
        // right.
        assert!(said.contains("[plugins.hermes]"), "{said}");
        assert!(said.contains("[plugins.discord]"), "{said}");
    }

    #[test]
    fn either_one_alone_and_either_one_switched_off_still_loads() {
        // The positive control: a refusal that fired on a lone durable log
        // would pass the test above and take the paper trail away entirely.
        for text in [
            "[plugins.hermes]\nenabled = true\n",
            &format!("[plugins.discord]\nenabled = true\ntype = \"bot\"\n{CATCH_ALL}"),
            "[plugins.hermes]\nenabled = true\n[plugins.discord]\nenabled = false\ntype = \"bot\"\n",
            &format!(
                "[plugins.hermes]\nenabled = false\n[plugins.discord]\nenabled = true\ntype = \"bot\"\n{CATCH_ALL}"
            ),
        ] {
            assert!(parse_config(text).is_ok(), "case: {text:?}");
        }
    }
}

#[cfg(test)]
mod catch_all_tests {
    use super::super::{ConfigError, parse_config};

    /// THE MUTANT THIS PINS: the requirement dropped, which loads a map whose
    /// unmapped projects post nowhere and say nothing.
    #[test]
    fn an_armed_map_with_no_default_is_refused_naming_the_table() {
        for text in [
            "[plugins.discord]\nenabled = true\ntype = \"bot\"\n",
            "[plugins.discord]\nenabled = true\ntype = \"bot\"\n[plugins.discord.channels]\ndotfiles = \"9001\"\n",
            "[plugins.discord]\nenabled = true\ntype = \"bot\"\n[plugins.discord.channels]\ndefault = \"\"\n",
        ] {
            let Err(ConfigError::Invalid(said)) = parse_config(text) else {
                panic!("case: {text:?}");
            };
            assert!(said.contains("[plugins.discord.channels]"), "{said}");
            assert!(said.contains("default"), "{said}");
        }
    }

    #[test]
    fn a_map_with_a_default_loads_and_a_switched_off_table_is_never_asked() {
        // The positive control: a refusal that fired on a table the operator
        // switched off would take the whole config down over a channel nobody
        // posts to.
        for text in [
            "[plugins.discord]\nenabled = true\ntype = \"bot\"\n[plugins.discord.channels]\ndefault = \"9001\"\npriority = \"9002\"\ndotfiles = \"9003\"\n",
            "[plugins.discord]\nenabled = false\ntype = \"bot\"\n",
        ] {
            assert!(parse_config(text).is_ok(), "case: {text:?}");
        }
    }
}
