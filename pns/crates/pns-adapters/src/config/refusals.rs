//! What a configuration file is REFUSED for, once the schema has admitted
//! every key in it.
//!
//! AT LOAD, WHICH BLOCKS THE WHOLE FILE, and that is the point of gathering
//! them here: each of these is a config that would otherwise load happily and
//! then misdeliver, one event at a time, in a way nobody notices. The file is
//! unusable until the named line is written, and the machine runs on its
//! built-in core meanwhile.

use super::*;

/// Refuses an ARMED discord log whose map states no catch-all.
///
/// `default` IS REQUIRED because every lookup ends there: a map without one
/// silently swallows the first event from every project nobody mapped, which
/// is the one failure a paper trail cannot afford. It is refused AT LOAD, with
/// the urgent-channel refusal beside it, so the file is unusable until the
/// line is written rather than each event failing one at a time.
///
/// A TABLE THAT IS OFF, OR NAMES NO BACKEND, IS NOT THIS REFUSAL'S BUSINESS:
/// the first is the operator's own switch and the second already has a
/// sentence of its own, which this one would only double.
pub(super) fn refuse_a_map_without_a_catch_all(config: &Config) -> Result<(), ConfigError> {
    let Some(settings) = armed_discord(config) else {
        return Ok(());
    };
    if super::states_default_channel(settings) {
        return Ok(());
    }
    Err(ConfigError::Invalid(
        "`[plugins.log.channels]` states no `default`: it is the catch-all every lookup \
         ends at, and a map without one swallows the first event from every project nobody \
         mapped. Write a `default` entry."
            .to_string(),
    ))
}

/// Refuses an ARMED discord log whose map states no channel for the urgent
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
    let Some(settings) = armed_discord(config) else {
        return Ok(());
    };
    let urgent = config.routes.urgent_route();
    if super::states_channel(settings, urgent) {
        return Ok(());
    }
    Err(ConfigError::Invalid(format!(
        "`[plugins.log.channels]` states no `{urgent}`: it is the channel every critical \
         page takes whatever project it was about, and a map without one sends those pages to \
         the project's own routine channel instead. Write a `{urgent}` entry."
    )))
}

/// The plugin tables the function-naming rename moved, each with the vendor
/// its `type` now names: the old heading, the new heading, the type.
///
/// ONE TABLE FOR BOTH REFUSALS BELOW, so the spelling an operator is told to
/// write and the value that heading then admits cannot drift apart.
const RENAMED_PLUGIN_TABLES: [(&str, &str, &str); 5] = [
    ("hue", "lights", "hue"),
    ("macos-banner", "banner", "macos"),
    ("router", "home_presence", "unifi"),
    ("hermes", "log", "hermes"),
    ("discord", "log", "discord"),
];

/// Refuses a config still holding a plugin table under its old heading,
/// naming the heading to write instead.
///
/// AT LOAD, WHICH BLOCKS THE WHOLE FILE, because the alternative is silence:
/// a plugin table nothing registered keeps its settings free-form, so the old
/// heading would load, arm nothing, and leave the operator with a destination
/// that stopped working and no line saying why.
pub(super) fn refuse_a_moved_plugin_table(config: &Config) -> Result<(), ConfigError> {
    let Some((old, new, vendor)) = RENAMED_PLUGIN_TABLES
        .iter()
        .find(|(old, _, _)| config.plugins.contains_key(*old))
    else {
        return Ok(());
    };
    Err(ConfigError::Invalid(format!(
        "`[plugins.{old}]` is now `[plugins.{new}]` with `type = \"{vendor}\"`: a plugin table \
         is named for the function it serves and `type` names the vendor behind it. Rename the \
         heading."
    )))
}

/// The two tables the rename gave a `type` to, each with the one vendor a
/// compiled-in backend answers.
///
/// `[plugins.home_presence]` IS NOT HERE: it has carried a `type` since before
/// the rename, and `router_settings` refuses its own by name so the probe's
/// diagnostic can report a router that is configured but unreachable;
/// refusing it here too would take that whole report away.
///
/// `[plugins.log]` IS NOT HERE EITHER: its `type` names which of two
/// transports carries the log rather than one vendor, and
/// `name_the_log_for_its_transport` settles it because the name the table is
/// filed under depends on the answer.
const SINGLE_VENDOR_TABLES: [(&str, &str); 2] = [("lights", "hue"), ("banner", "macos")];

/// Refuses an ARMED `[plugins.lights]` or `[plugins.banner]` whose `type`
/// names a vendor nothing compiled in answers, naming the one that is
/// accepted.
///
/// AN ABSENT `type` IS THE ONE COMPILED-IN VENDOR, which is what the shipped
/// file writes: these two tables gained the key in the rename, so a file
/// without it is a file written before the key existed rather than one naming
/// a backend nothing answers.
pub(super) fn refuse_a_plugin_type_nothing_answers(config: &Config) -> Result<(), ConfigError> {
    for (table, vendor) in SINGLE_VENDOR_TABLES {
        let Some(entry) = config.plugins.get(table).filter(|entry| entry.enabled) else {
            continue;
        };
        let Some(named) = entry.settings.get("type").and_then(toml::Value::as_str) else {
            continue;
        };
        if named != vendor {
            return Err(ConfigError::Invalid(format!(
                "[plugins.{table}] has type {named:?}, which no compiled-in backend answers; \
                 the only type is {vendor:?}"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod renamed_table_tests {
    use super::super::{ConfigError, parse_config};

    #[test]
    fn a_config_still_holding_an_old_plugin_heading_is_refused_naming_the_new_one() {
        for (old, new) in [
            ("hue", "lights"),
            ("macos-banner", "banner"),
            ("router", "home_presence"),
        ] {
            let Err(ConfigError::Invalid(said)) =
                parse_config(&format!("[plugins.{old}]\nenabled = true\n"))
            else {
                panic!("`[plugins.{old}]` was accepted");
            };
            assert!(said.contains(old), "{said}");
            assert!(said.contains(new), "{said}");
        }
    }

    #[test]
    fn a_renamed_table_naming_a_vendor_nothing_answers_is_refused_naming_the_one_that_is() {
        for (table, vendor) in [("lights", "hue"), ("banner", "macos")] {
            let Err(ConfigError::Invalid(said)) = parse_config(&format!(
                "[plugins.{table}]\nenabled = true\ntype = \"aqara\"\n"
            )) else {
                panic!("`[plugins.{table}]` took a type nothing answers");
            };
            assert!(said.contains("aqara"), "{said}");
            assert!(said.contains(vendor), "{said}");
        }
    }

    #[test]
    fn the_renamed_tables_arm_their_plugins_under_their_own_vendor() {
        for (table, vendor) in [
            ("lights", "hue"),
            ("banner", "macos"),
            ("home_presence", "unifi"),
        ] {
            let config = parse_config(&format!(
                "[plugins.{table}]\nenabled = true\ntype = \"{vendor}\"\n"
            ))
            .unwrap_or_else(|error| panic!("`[plugins.{table}]` was refused: {error:?}"));
            assert!(config.plugins[table].enabled, "`[plugins.{table}]` is off");
        }
    }
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
                    "[plugins.log]\nenabled = true\ntype = \"discord\"\n\
                     [plugins.log.channels]\ndefault = \"{FAKE}\"\ndotfiles = \"{FAKE}\"\n"
                ),
                "priority",
            ),
            (
                format!(
                    "[routes]\nurgent = \"sirens\"\n\
                     [plugins.log]\nenabled = true\ntype = \"discord\"\n\
                     [plugins.log.channels]\ndefault = \"{FAKE}\"\n"
                ),
                "sirens",
            ),
            (
                format!(
                    "[plugins.log]\nenabled = true\ntype = \"discord\"\n\
                     [plugins.log.channels]\ndefault = \"{FAKE}\"\npriority = \"\"\n"
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
            assert!(said.contains("[plugins.log.channels]"), "{said}");
        }
    }

    #[test]
    fn a_map_naming_the_urgent_route_loads_and_a_switched_off_table_is_never_asked() {
        // The positive control: a refusal that fired on either of these would
        // take the whole config down over a channel the operator did arm, or
        // over one nobody posts to.
        for text in [
            format!(
                "[plugins.log]\nenabled = true\ntype = \"discord\"\n\
                 [plugins.log.channels]\ndefault = \"{FAKE}\"\npriority = \"{FAKE}\"\n"
            ),
            format!(
                "[routes]\nurgent = \"sirens\"\n\
                 [plugins.log]\nenabled = true\ntype = \"discord\"\n\
                 [plugins.log.channels]\ndefault = \"{FAKE}\"\nsirens = \"{FAKE}\"\n"
            ),
            "[plugins.log]\nenabled = false\ntype = \"discord\"\n".to_string(),
        ] {
            assert!(parse_config(&text).is_ok(), "case: {text:?}");
        }
    }
}

#[cfg(test)]
mod durable_log_tests {
    use super::super::{ConfigError, parse_config};

    /// The two entries an armed map cannot load without: the catch-all every
    /// lookup ends at, and the channel the urgent route takes.
    const CATCH_ALL: &str = "[plugins.log.channels]\ndefault = \"9001\"\npriority = \"9002\"\n";

    /// THE MUTANT THIS PINS: a `type` admitted without being checked, which
    /// files the durable log under a name the roster never registered and
    /// takes the paper trail away with no line saying so.
    #[test]
    fn a_type_no_transport_answers_is_refused_naming_the_ones_that_are() {
        for text in [
            "[plugins.log]\nenabled = true\ntype = \"telegram\"\n",
            "[plugins.log]\nenabled = true\n",
            "[plugins.log]\nenabled = false\ntype = \"\"\n",
        ] {
            let Err(ConfigError::Invalid(said)) = parse_config(text) else {
                panic!("case: {text:?}");
            };
            assert!(said.contains("[plugins.log]"), "{said}");
            // BOTH TRANSPORTS, because the operator has to pick one and the
            // sentence refusing their spelling is where the two are.
            assert!(said.contains("\"hermes\""), "{said}");
            assert!(said.contains("\"discord\""), "{said}");
        }
    }

    /// Each transport arms the durable log under its own name, which is the
    /// name the roster registers and every leg is recorded under.
    #[test]
    fn each_transport_arms_the_durable_log_under_its_own_name() {
        for (transport, extra) in [("hermes", ""), ("discord", CATCH_ALL)] {
            let config = parse_config(&format!(
                "[plugins.log]\nenabled = true\ntype = \"{transport}\"\n{extra}"
            ))
            .unwrap_or_else(|error| panic!("`{transport}` was refused: {error:?}"));
            assert!(config.plugins[transport].enabled, "{transport} is off");
            // AND UNDER NOTHING ELSE: one table means the other transport is
            // not declared at all, which is what makes two durable logs
            // unrepresentable rather than refused.
            assert!(!config.plugins.contains_key("log"), "{transport}");
            assert_eq!(config.plugins.len(), 1, "{transport}");
        }
    }

    /// THE MUTANT THIS PINS: the old headings quietly admitted again, which
    /// is a table nothing registers and a durable log that stopped posting.
    #[test]
    fn the_old_durable_log_headings_are_refused_naming_the_new_one() {
        for (old, transport) in [("hermes", "hermes"), ("discord", "discord")] {
            let Err(ConfigError::Invalid(said)) =
                parse_config(&format!("[plugins.{old}]\nenabled = true\n"))
            else {
                panic!("`[plugins.{old}]` was accepted");
            };
            assert!(said.contains("[plugins.log]"), "{said}");
            assert!(said.contains(transport), "{said}");
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
            "[plugins.log]\nenabled = true\ntype = \"discord\"\n",
            "[plugins.log]\nenabled = true\ntype = \"discord\"\n[plugins.log.channels]\ndotfiles = \"9001\"\n",
            "[plugins.log]\nenabled = true\ntype = \"discord\"\n[plugins.log.channels]\ndefault = \"\"\n",
        ] {
            let Err(ConfigError::Invalid(said)) = parse_config(text) else {
                panic!("case: {text:?}");
            };
            assert!(said.contains("[plugins.log.channels]"), "{said}");
            assert!(said.contains("default"), "{said}");
        }
    }

    #[test]
    fn a_map_with_a_default_loads_and_a_switched_off_table_is_never_asked() {
        // The positive control: a refusal that fired on a table the operator
        // switched off would take the whole config down over a channel nobody
        // posts to.
        for text in [
            "[plugins.log]\nenabled = true\ntype = \"discord\"\n[plugins.log.channels]\ndefault = \"9001\"\npriority = \"9002\"\ndotfiles = \"9003\"\n",
            "[plugins.log]\nenabled = false\ntype = \"discord\"\n",
        ] {
            assert!(parse_config(text).is_ok(), "case: {text:?}");
        }
    }
}
